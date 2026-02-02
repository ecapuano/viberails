use std::{fs, io::Write, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use log::{info, warn};
use serde_json::{Value, json};

use crate::common::PROJECT_NAME;
use crate::hooks::binary_location;
use crate::providers::discovery::{DiscoveryResult, ProviderDiscovery, ProviderFactory};
use crate::providers::{HookEntry, LLmProviderTrait};

/// Clawdbot (now `OpenClaw`) uses a hooks system with directory-based handlers.
/// We create a hook directory with HOOK.md and handler.ts files.
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

    /// Get all detected installation directories (for installing hooks in both if present)
    fn all_detected_dirs() -> Vec<(PathBuf, &'static str)> {
        let mut dirs = Vec::new();
        if let Some(home) = dirs::home_dir() {
            let openclaw = home.join(".openclaw");
            if openclaw.exists() {
                dirs.push((openclaw, "openclaw.json"));
            }
            let clawdbot = home.join(".clawdbot");
            if clawdbot.exists() {
                dirs.push((clawdbot, "clawdbot.json"));
            }
        }
        dirs
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
            hooks_installed: false, // Will be set by discover_with_hooks_check
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
    /// Path to the viberails binary
    binary_path: PathBuf,
}

impl Clawdbot {
    pub fn new() -> Result<Self> {
        // Always use the installed binary location (~/.local/bin/viberails) rather than
        // current_exe(), so the hook command is consistent regardless of where viberails
        // is run from. This prevents duplicate hooks when running from different locations.
        let exe = binary_location()?;
        Ok(Self::with_custom_path(exe))
    }

    pub fn with_custom_path<P: AsRef<std::path::Path>>(program: P) -> Self {
        Self {
            binary_path: program.as_ref().to_path_buf(),
        }
    }

    /// Get the hook directory path for a given installation directory.
    fn hook_dir(install_dir: &std::path::Path) -> PathBuf {
        install_dir.join("hooks").join(PROJECT_NAME)
    }

    /// Generate the HOOK.md content
    pub(crate) fn generate_hook_md() -> String {
        format!(
            r#"---
name: {PROJECT_NAME}
description: "{PROJECT_NAME} security and compliance hook"
metadata:
  openclaw:
    emoji: "🛡️"
    events: ["command"]
    requires:
      bins: ["{PROJECT_NAME}"]
---

# {PROJECT_NAME} Hook

This hook integrates {PROJECT_NAME} with Clawdbot/OpenClaw for security and compliance monitoring.

All command events are forwarded to {PROJECT_NAME} for policy evaluation.
"#
        )
    }

    /// Generate the handler.ts content
    pub(crate) fn generate_handler_ts(&self) -> String {
        let binary_path = self.binary_path.display();
        format!(
            r#"import type {{ HookHandler }} from "openclaw/hooks";
import {{ spawn }} from "child_process";

const handler: HookHandler = async (event) => {{
  // Forward all events to {PROJECT_NAME} for processing
  const eventJson = JSON.stringify(event);

  return new Promise((resolve, reject) => {{
    const proc = spawn("{binary_path}", ["clawdbot-callback"], {{
      stdio: ["pipe", "pipe", "pipe"],
    }});

    let stdout = "";
    let stderr = "";

    proc.stdout.on("data", (data: Buffer) => {{
      stdout += data.toString();
    }});

    proc.stderr.on("data", (data: Buffer) => {{
      stderr += data.toString();
    }});

    proc.on("close", (code: number | null) => {{
      if (code !== 0) {{
        console.error(`[{PROJECT_NAME}] Process exited with code ${{code}}: ${{stderr}}`);
      }}

      // Parse response if available
      if (stdout.trim()) {{
        try {{
          const response = JSON.parse(stdout.trim());
          if (response.decision === "block" && response.reason) {{
            event.messages.push(`🛡️ {PROJECT_NAME}: ${{response.reason}}`);
          }}
        }} catch (e) {{
          // Ignore parse errors, response may be empty for non-tool events
        }}
      }}
      resolve();
    }});

    proc.on("error", (err: Error) => {{
      console.error(`[{PROJECT_NAME}] Failed to spawn process: ${{err.message}}`);
      resolve(); // Don't block on spawn errors
    }});

    // Send event data to stdin
    proc.stdin.write(eventJson);
    proc.stdin.end();
  }});
}};

export default handler;
"#
        )
    }

    /// Install hook files into a specific directory
    fn install_hook_files(&self, install_dir: &std::path::Path) -> Result<()> {
        let hook_dir = Self::hook_dir(install_dir);

        // Create hook directory
        fs::create_dir_all(&hook_dir).with_context(|| {
            format!("Unable to create hook directory at {}", hook_dir.display())
        })?;

        // Write HOOK.md
        let hook_md_path = hook_dir.join("HOOK.md");
        fs::write(&hook_md_path, Self::generate_hook_md()).with_context(|| {
            format!("Unable to write HOOK.md at {}", hook_md_path.display())
        })?;

        // Write handler.ts
        let handler_ts_path = hook_dir.join("handler.ts");
        fs::write(&handler_ts_path, self.generate_handler_ts()).with_context(|| {
            format!("Unable to write handler.ts at {}", handler_ts_path.display())
        })?;

        info!(
            "Installed {PROJECT_NAME} hook files at {}",
            hook_dir.display()
        );

        Ok(())
    }

    /// Enable the hook in the config file
    fn enable_in_config(config_file: &std::path::Path) -> Result<()> {
        // Ensure config file exists
        if let Some(parent) = config_file.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Unable to create config directory at {}",
                    parent.display()
                )
            })?;
        }

        if !config_file.exists() {
            fs::write(config_file, "{}\n").with_context(|| {
                format!("Unable to create config file at {}", config_file.display())
            })?;
        }

        // Read and parse config
        let data = fs::read_to_string(config_file)
            .with_context(|| format!("Unable to read {}", config_file.display()))?;

        let mut json: Value = serde_json::from_str(&data)
            .with_context(|| format!("Unable to parse JSON in {}", config_file.display()))?;

        // Update config structure
        let root = json.as_object_mut().ok_or_else(|| {
            anyhow!(
                "Expected root of {} to be a JSON object",
                config_file.display()
            )
        })?;

        let hooks_obj = root
            .entry("hooks")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| {
                anyhow!(
                    "Expected 'hooks' field in {} to be an object",
                    config_file.display()
                )
            })?;

        let internal_obj = hooks_obj
            .entry("internal")
            .or_insert_with(|| json!({"enabled": true, "entries": {}}))
            .as_object_mut()
            .ok_or_else(|| {
                anyhow!(
                    "Expected 'hooks.internal' field in {} to be an object",
                    config_file.display()
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
                    config_file.display()
                )
            })?;

        // Check if hook is already enabled
        if entries_obj.get(PROJECT_NAME).is_some() {
            warn!(
                "{PROJECT_NAME} hook already enabled in {}",
                config_file.display()
            );
        } else {
            // Add hook entry - only "enabled" is valid, no "command" key!
            entries_obj.insert(PROJECT_NAME.to_string(), json!({ "enabled": true }));
        }

        // Write back with trailing newline
        let mut json_str =
            serde_json::to_string_pretty(&json).context("Failed to serialize config")?;
        json_str.push('\n');

        let mut fd = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(config_file)
            .with_context(|| format!("Unable to open {} for writing", config_file.display()))?;

        fd.write_all(json_str.as_bytes())
            .with_context(|| format!("Failed to write to {}", config_file.display()))?;

        info!("Enabled {PROJECT_NAME} hook in {}", config_file.display());

        Ok(())
    }

    /// Remove hook files from a specific directory
    fn uninstall_hook_files(install_dir: &std::path::Path) -> Result<()> {
        let hook_dir = Self::hook_dir(install_dir);

        if hook_dir.exists() {
            fs::remove_dir_all(&hook_dir).with_context(|| {
                format!("Unable to remove hook directory at {}", hook_dir.display())
            })?;
            info!(
                "Removed {PROJECT_NAME} hook files from {}",
                hook_dir.display()
            );
        }

        Ok(())
    }

    /// Disable the hook in the config file
    fn disable_in_config(config_file: &std::path::Path) -> Result<()> {
        if !config_file.exists() {
            return Ok(());
        }

        let data = fs::read_to_string(config_file)
            .with_context(|| format!("Unable to read {}", config_file.display()))?;

        let mut json: Value = serde_json::from_str(&data)
            .with_context(|| format!("Unable to parse JSON in {}", config_file.display()))?;

        // Remove entry from config
        let removed = json
            .as_object_mut()
            .and_then(|root| root.get_mut("hooks"))
            .and_then(|h| h.as_object_mut())
            .and_then(|h| h.get_mut("internal"))
            .and_then(|i| i.as_object_mut())
            .and_then(|i| i.get_mut("entries"))
            .and_then(|e| e.as_object_mut())
            .and_then(|entries| entries.remove(PROJECT_NAME));

        if removed.is_none() {
            warn!(
                "{PROJECT_NAME} hook not found in {}",
                config_file.display()
            );
            return Ok(());
        }

        // Write back with trailing newline
        let mut json_str =
            serde_json::to_string_pretty(&json).context("Failed to serialize config")?;
        json_str.push('\n');

        let mut fd = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(config_file)
            .with_context(|| format!("Unable to open {} for writing", config_file.display()))?;

        fd.write_all(json_str.as_bytes())
            .with_context(|| format!("Failed to write to {}", config_file.display()))?;

        info!(
            "Disabled {PROJECT_NAME} hook in {}",
            config_file.display()
        );

        Ok(())
    }

    /// For testing: install into a specific config and update its JSON
    #[cfg(test)]
    pub(crate) fn install_into(&self, _hook_type: &str, json: &mut Value) -> Result<()> {
        let root = json.as_object_mut().ok_or_else(|| {
            anyhow!("Expected root to be a JSON object")
        })?;

        let hooks_obj = root
            .entry("hooks")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| anyhow!("Expected 'hooks' to be an object"))?;

        let internal_obj = hooks_obj
            .entry("internal")
            .or_insert_with(|| json!({"enabled": true, "entries": {}}))
            .as_object_mut()
            .ok_or_else(|| anyhow!("Expected 'hooks.internal' to be an object"))?;

        internal_obj.insert("enabled".to_string(), json!(true));

        let entries_obj = internal_obj
            .entry("entries")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| anyhow!("Expected 'hooks.internal.entries' to be an object"))?;

        // Only add "enabled" - no "command" key!
        if entries_obj.get(PROJECT_NAME).is_none() {
            entries_obj.insert(PROJECT_NAME.to_string(), json!({ "enabled": true }));
        }

        Ok(())
    }

    /// For testing: uninstall from a specific JSON
    #[cfg(test)]
    pub(crate) fn uninstall_from(&self, _hook_type: &str, json: &mut Value) {
        let _ = json
            .as_object_mut()
            .and_then(|root| root.get_mut("hooks"))
            .and_then(|h| h.as_object_mut())
            .and_then(|h| h.get_mut("internal"))
            .and_then(|i| i.as_object_mut())
            .and_then(|i| i.get_mut("entries"))
            .and_then(|e| e.as_object_mut())
            .and_then(|entries| entries.remove(PROJECT_NAME));
    }
}

impl LLmProviderTrait for Clawdbot {
    fn name(&self) -> &'static str {
        "clawdbot"
    }

    fn install(&self, hook_type: &str) -> Result<()> {
        let detected_dirs = ClawdbotDiscovery::all_detected_dirs();

        if detected_dirs.is_empty() {
            anyhow::bail!("No Clawdbot/OpenClaw installation detected");
        }

        // Install into all detected installations (both openclaw and clawdbot if present)
        for (install_dir, config_name) in detected_dirs {
            info!(
                "Installing {hook_type} in {}",
                install_dir.display()
            );

            // Install hook files (HOOK.md and handler.ts)
            self.install_hook_files(&install_dir)?;

            // Enable in config
            let config_file = install_dir.join(config_name);
            Self::enable_in_config(&config_file)?;
        }

        Ok(())
    }

    fn uninstall(&self, hook_type: &str) -> Result<()> {
        let detected_dirs = ClawdbotDiscovery::all_detected_dirs();

        // Uninstall from all detected installations
        for (install_dir, config_name) in detected_dirs {
            info!(
                "Uninstalling {hook_type} from {}",
                install_dir.display()
            );

            // Remove hook files
            Self::uninstall_hook_files(&install_dir)?;

            // Disable in config
            let config_file = install_dir.join(config_name);
            Self::disable_in_config(&config_file)?;
        }

        Ok(())
    }

    fn list(&self) -> Result<Vec<HookEntry>> {
        let mut entries = Vec::new();

        for (install_dir, config_name) in ClawdbotDiscovery::all_detected_dirs() {
            let config_file = install_dir.join(config_name);

            if !config_file.exists() {
                continue;
            }

            let data = fs::read_to_string(&config_file)
                .with_context(|| format!("Unable to read {}", config_file.display()))?;

            let json: Value = serde_json::from_str(&data)
                .with_context(|| format!("Unable to parse JSON in {}", config_file.display()))?;

            let entries_obj = json
                .get("hooks")
                .and_then(|h| h.get("internal"))
                .and_then(|i| i.get("entries"))
                .and_then(|e| e.as_object());

            let Some(entries_obj) = entries_obj else {
                continue;
            };

            for (hook_name, hook_config) in entries_obj {
                let enabled = hook_config
                    .get("enabled")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true);

                // Check if hook directory exists
                let hook_dir = install_dir.join("hooks").join(hook_name);
                let has_handler = hook_dir.join("handler.ts").exists();

                let command = if has_handler {
                    format!("[directory hook: {}]", hook_dir.display())
                } else {
                    "[directory hook - handler missing]".to_string()
                };

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
        }

        Ok(entries)
    }
}
