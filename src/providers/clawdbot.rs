use std::{fs, io::Write, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use log::{info, warn};
use serde_json::{Value, json};

use crate::common::PROJECT_NAME;
use crate::hooks::binary_location;
use crate::providers::discovery::{DiscoveryResult, ProviderDiscovery, ProviderFactory};
use crate::providers::{HookEntry, LLmProviderTrait};

/// Clawdbot (now `OpenClaw`) uses a plugin system for tool interception.
/// We create a plugin in the extensions directory with a manifest and handler.
/// Note: Event-stream hooks (HOOK.md with `events: ["command"]`) only cover slash
/// commands like /new, /reset, /stop. For tool call interception, we need
/// plugin hooks (`before_tool_call`, `after_tool_call`) which are registered
/// programmatically through the plugin system.
pub const CLAWDBOT_HOOKS: &[&str] = &["plugin"];

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

    /// Get the plugin directory path for a given installation directory.
    fn plugin_dir(install_dir: &std::path::Path) -> PathBuf {
        install_dir.join("extensions").join(PROJECT_NAME)
    }

    /// Get the legacy hook directory path (for cleanup during migration).
    fn legacy_hook_dir(install_dir: &std::path::Path) -> PathBuf {
        install_dir.join("hooks").join(PROJECT_NAME)
    }

    /// Generate the plugin manifest JSON content.
    /// Uses the appropriate manifest filename based on whether this is
    /// `OpenClaw` (`openclaw.plugin.json`) or legacy Clawdbot (`clawdbot.plugin.json`).
    pub(crate) fn generate_plugin_manifest() -> String {
        serde_json::to_string_pretty(&json!({
            "id": PROJECT_NAME,
            "name": format!("{} Security Plugin", PROJECT_NAME),
            "version": "1.0.0",
            "description": format!("{} security and compliance monitoring for tool calls", PROJECT_NAME),
            "main": "index.ts"
        }))
        .unwrap_or_default()
    }

    /// Generate the plugin index.ts content that registers tool call hooks.
    /// This uses the plugin API to register `before_tool_call` hooks, which
    /// intercept tool calls before they are executed (unlike event-stream
    /// hooks which only cover slash commands).
    pub(crate) fn generate_plugin_index(&self) -> String {
        let binary_path = self.binary_path.display();
        format!(
            r#"import {{ spawnSync }} from "child_process";

/**
 * {PROJECT_NAME} Plugin for OpenClaw/Clawdbot
 *
 * This plugin intercepts tool calls (exec, Read, Write, etc.) for security
 * and compliance monitoring. Unlike event-stream hooks (which only cover
 * slash commands like /new, /reset), plugin hooks can intercept actual
 * tool executions.
 */

interface ToolCallEvent {{
  tool: string;
  parameters: Record<string, unknown>;
  sessionId?: string;
}}

interface ToolCallResult {{
  allow: boolean;
  reason?: string;
  modifiedParameters?: Record<string, unknown>;
}}

interface PluginAPI {{
  registerHook(hookName: string, handler: (event: unknown) => unknown): void;
  // Legacy clawdbot uses addHook instead of registerHook
  addHook?(hookName: string, handler: (event: unknown) => unknown): void;
}}

function callViberails(event: ToolCallEvent): ToolCallResult {{
  const eventJson = JSON.stringify(event);

  try {{
    const result = spawnSync("{binary_path}", ["clawdbot-callback"], {{
      input: eventJson,
      encoding: "utf-8",
      timeout: 30000, // 30 second timeout
    }});

    if (result.error) {{
      console.error(`[{PROJECT_NAME}] Spawn error: ${{result.error.message}}`);
      return {{ allow: true }}; // Fail open on spawn errors
    }}

    if (result.status !== 0) {{
      console.error(`[{PROJECT_NAME}] Process exited with code ${{result.status}}: ${{result.stderr}}`);
      return {{ allow: true }}; // Fail open on process errors
    }}

    if (result.stdout?.trim()) {{
      try {{
        const response = JSON.parse(result.stdout.trim());
        if (response.decision === "block") {{
          return {{
            allow: false,
            reason: response.reason || "Blocked by {PROJECT_NAME} policy",
          }};
        }}
      }} catch (e) {{
        // Ignore parse errors, response may be empty for allowed actions
      }}
    }}

    return {{ allow: true }};
  }} catch (e) {{
    console.error(`[{PROJECT_NAME}] Exception: ${{e}}`);
    return {{ allow: true }}; // Fail open on exceptions
  }}
}}

export default function register(api: PluginAPI) {{
  // Register before_tool_call hook to intercept tool executions
  // This fires before any tool (exec, Read, Write, etc.) is executed
  const hookHandler = (event: unknown) => {{
    const toolEvent = event as ToolCallEvent;

    // Skip internal/system tools if needed
    if (!toolEvent.tool) {{
      return event;
    }}

    const result = callViberails(toolEvent);

    if (!result.allow) {{
      // Return a blocked response - the exact format depends on OpenClaw/Clawdbot version
      // OpenClaw expects throwing or returning an error object
      throw new Error(`🛡️ {PROJECT_NAME}: ${{result.reason}}`);
    }}

    // Return the original or modified event to allow the tool call to proceed
    return result.modifiedParameters ? {{ ...toolEvent, parameters: result.modifiedParameters }} : event;
  }};

  // Try OpenClaw API first, fall back to legacy Clawdbot API
  if (typeof api.registerHook === "function") {{
    api.registerHook("before_tool_call", hookHandler);
  }} else if (typeof api.addHook === "function") {{
    api.addHook("before_tool_call", hookHandler);
  }} else {{
    console.warn("[{PROJECT_NAME}] Could not register hook: API method not found");
  }}

  console.log(`[{PROJECT_NAME}] Plugin loaded - monitoring tool calls`);
}}
"#
        )
    }

    /// Install plugin files into a specific directory.
    /// Also cleans up legacy hook files if they exist.
    fn install_plugin_files(&self, install_dir: &std::path::Path, is_openclaw: bool) -> Result<()> {
        // Clean up legacy hook directory if it exists (migration from old hook-based approach)
        let legacy_dir = Self::legacy_hook_dir(install_dir);
        if legacy_dir.exists() {
            info!(
                "Removing legacy hook directory at {} (migrating to plugin system)",
                legacy_dir.display()
            );
            let _ = fs::remove_dir_all(&legacy_dir);
        }

        let plugin_dir = Self::plugin_dir(install_dir);

        // Create plugin directory
        fs::create_dir_all(&plugin_dir).with_context(|| {
            format!(
                "Unable to create plugin directory at {}",
                plugin_dir.display()
            )
        })?;

        // Write plugin manifest with appropriate filename
        let manifest_name = if is_openclaw {
            "openclaw.plugin.json"
        } else {
            "clawdbot.plugin.json"
        };
        let manifest_path = plugin_dir.join(manifest_name);
        fs::write(&manifest_path, Self::generate_plugin_manifest()).with_context(|| {
            format!("Unable to write {} at {}", manifest_name, manifest_path.display())
        })?;

        // Write index.ts
        let index_path = plugin_dir.join("index.ts");
        fs::write(&index_path, self.generate_plugin_index()).with_context(|| {
            format!("Unable to write index.ts at {}", index_path.display())
        })?;

        info!(
            "Installed {PROJECT_NAME} plugin at {}",
            plugin_dir.display()
        );

        Ok(())
    }

    /// Enable the plugin in the config file.
    /// Plugins are registered under "plugins.entries" in the config.
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

        // Update config structure for plugins
        let root = json.as_object_mut().ok_or_else(|| {
            anyhow!(
                "Expected root of {} to be a JSON object",
                config_file.display()
            )
        })?;

        // Also clean up old hook entries if they exist (migration)
        if let Some(hooks) = root.get_mut("hooks")
            && let Some(internal) = hooks.get_mut("internal")
            && let Some(entries) = internal.get_mut("entries")
            && let Some(entries_obj) = entries.as_object_mut()
            && entries_obj.remove(PROJECT_NAME).is_some()
        {
            info!(
                "Removed legacy hook entry from {} (migrating to plugin)",
                config_file.display()
            );
        }

        // Add plugin entry
        let plugins_obj = root
            .entry("plugins")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| {
                anyhow!(
                    "Expected 'plugins' field in {} to be an object",
                    config_file.display()
                )
            })?;

        let entries_obj = plugins_obj
            .entry("entries")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| {
                anyhow!(
                    "Expected 'plugins.entries' field in {} to be an object",
                    config_file.display()
                )
            })?;

        // Check if plugin is already enabled
        if entries_obj.get(PROJECT_NAME).is_some() {
            warn!(
                "{PROJECT_NAME} plugin already enabled in {}",
                config_file.display()
            );
        } else {
            // Add plugin entry
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

        info!(
            "Enabled {PROJECT_NAME} plugin in {}",
            config_file.display()
        );

        Ok(())
    }

    /// Remove plugin files from a specific directory.
    /// Also removes legacy hook files if they exist.
    fn uninstall_plugin_files(install_dir: &std::path::Path) -> Result<()> {
        // Remove plugin directory
        let plugin_dir = Self::plugin_dir(install_dir);
        if plugin_dir.exists() {
            fs::remove_dir_all(&plugin_dir).with_context(|| {
                format!(
                    "Unable to remove plugin directory at {}",
                    plugin_dir.display()
                )
            })?;
            info!(
                "Removed {PROJECT_NAME} plugin from {}",
                plugin_dir.display()
            );
        }

        // Also remove legacy hook directory if it exists
        let legacy_dir = Self::legacy_hook_dir(install_dir);
        if legacy_dir.exists() {
            let _ = fs::remove_dir_all(&legacy_dir);
            info!(
                "Removed legacy {PROJECT_NAME} hook from {}",
                legacy_dir.display()
            );
        }

        Ok(())
    }

    /// Disable the plugin in the config file.
    /// Also cleans up legacy hook entries if they exist.
    fn disable_in_config(config_file: &std::path::Path) -> Result<()> {
        if !config_file.exists() {
            return Ok(());
        }

        let data = fs::read_to_string(config_file)
            .with_context(|| format!("Unable to read {}", config_file.display()))?;

        let mut json: Value = serde_json::from_str(&data)
            .with_context(|| format!("Unable to parse JSON in {}", config_file.display()))?;

        let mut modified = false;

        // Remove from plugins.entries
        let removed_plugin = json
            .as_object_mut()
            .and_then(|root| root.get_mut("plugins"))
            .and_then(|p| p.as_object_mut())
            .and_then(|p| p.get_mut("entries"))
            .and_then(|e| e.as_object_mut())
            .and_then(|entries| entries.remove(PROJECT_NAME));

        if removed_plugin.is_some() {
            modified = true;
        }

        // Also remove legacy hook entry if it exists
        let removed_hook = json
            .as_object_mut()
            .and_then(|root| root.get_mut("hooks"))
            .and_then(|h| h.as_object_mut())
            .and_then(|h| h.get_mut("internal"))
            .and_then(|i| i.as_object_mut())
            .and_then(|i| i.get_mut("entries"))
            .and_then(|e| e.as_object_mut())
            .and_then(|entries| entries.remove(PROJECT_NAME));

        if removed_hook.is_some() {
            modified = true;
            info!(
                "Removed legacy hook entry from {}",
                config_file.display()
            );
        }

        if !modified {
            warn!(
                "{PROJECT_NAME} not found in {}",
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
            "Disabled {PROJECT_NAME} plugin in {}",
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

        // Add plugin entry (new format)
        let plugins_obj = root
            .entry("plugins")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| anyhow!("Expected 'plugins' to be an object"))?;

        let entries_obj = plugins_obj
            .entry("entries")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| anyhow!("Expected 'plugins.entries' to be an object"))?;

        if entries_obj.get(PROJECT_NAME).is_none() {
            entries_obj.insert(PROJECT_NAME.to_string(), json!({ "enabled": true }));
        }

        Ok(())
    }

    /// For testing: uninstall from a specific JSON
    #[cfg(test)]
    pub(crate) fn uninstall_from(&self, _hook_type: &str, json: &mut Value) {
        // Remove from plugins.entries (new format)
        let _ = json
            .as_object_mut()
            .and_then(|root| root.get_mut("plugins"))
            .and_then(|p| p.as_object_mut())
            .and_then(|p| p.get_mut("entries"))
            .and_then(|e| e.as_object_mut())
            .and_then(|entries| entries.remove(PROJECT_NAME));

        // Also remove from legacy hooks.internal.entries if present
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
            let is_openclaw = config_name == "openclaw.json";
            info!(
                "Installing {hook_type} plugin in {}",
                install_dir.display()
            );

            // Install plugin files (manifest and index.ts)
            self.install_plugin_files(&install_dir, is_openclaw)?;

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

            // Remove plugin files (and legacy hook files)
            Self::uninstall_plugin_files(&install_dir)?;

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

            // Check plugins.entries (new format)
            if let Some(entries_obj) = json
                .get("plugins")
                .and_then(|p| p.get("entries"))
                .and_then(|e| e.as_object())
            {
                for (plugin_name, plugin_config) in entries_obj {
                    let enabled = plugin_config
                        .get("enabled")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(true);

                    // Check if plugin directory exists
                    let plugin_dir = install_dir.join("extensions").join(plugin_name);
                    let has_index = plugin_dir.join("index.ts").exists();

                    let command = if has_index {
                        format!("[plugin: {}]", plugin_dir.display())
                    } else {
                        "[plugin - index.ts missing]".to_string()
                    };

                    entries.push(HookEntry {
                        hook_type: "plugin".to_string(),
                        matcher: if enabled {
                            plugin_name.clone()
                        } else {
                            format!("{plugin_name} (disabled)")
                        },
                        command,
                    });
                }
            }

            // Also check legacy hooks.internal.entries format
            if let Some(entries_obj) = json
                .get("hooks")
                .and_then(|h| h.get("internal"))
                .and_then(|i| i.get("entries"))
                .and_then(|e| e.as_object())
            {
                for (hook_name, hook_config) in entries_obj {
                    let enabled = hook_config
                        .get("enabled")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(true);

                    // Check if hook directory exists
                    let hook_dir = install_dir.join("hooks").join(hook_name);
                    let has_handler = hook_dir.join("handler.ts").exists();

                    let command = if has_handler {
                        format!("[legacy hook: {}]", hook_dir.display())
                    } else {
                        "[legacy hook - handler missing]".to_string()
                    };

                    entries.push(HookEntry {
                        hook_type: "internal (legacy)".to_string(),
                        matcher: if enabled {
                            hook_name.clone()
                        } else {
                            format!("{hook_name} (disabled)")
                        },
                        command,
                    });
                }
            }
        }

        Ok(entries)
    }
}
