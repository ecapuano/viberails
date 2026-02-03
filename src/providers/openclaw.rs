use std::{fs, io::Write, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use log::{info, warn};
use serde_json::{Value, json};

use crate::common::PROJECT_NAME;
use crate::hooks::binary_location;
use crate::providers::discovery::{DiscoveryResult, ProviderDiscovery, ProviderFactory};
use crate::providers::{HookEntry, LLmProviderTrait};

/// `OpenClaw` uses a plugin system for tool interception via the `before_tool_call` hook.
/// We create a plugin in the extensions directory with a manifest and handler.
/// Ref: <https://docs.openclaw.ai/plugin>
pub const OPENCLAW_HOOKS: &[&str] = &["plugin"];

/// Discovery implementation for `OpenClaw`.
pub struct OpenClawDiscovery;

impl OpenClawDiscovery {
    /// Get the `OpenClaw` config directory and config file name.
    /// Returns (`directory_path`, `config_file_name`)
    fn openclaw_paths() -> Option<(PathBuf, &'static str)> {
        dirs::home_dir().map(|h| (h.join(".openclaw"), "openclaw.json"))
    }

    /// Get the `OpenClaw` config directory path.
    fn openclaw_dir() -> Option<PathBuf> {
        Self::openclaw_paths().map(|(dir, _)| dir)
    }

    /// Check if openclaw directory exists
    fn is_detected() -> bool {
        dirs::home_dir().is_some_and(|h| h.join(".openclaw").exists())
    }
}

impl ProviderDiscovery for OpenClawDiscovery {
    fn id(&self) -> &'static str {
        "openclaw"
    }

    fn display_name(&self) -> &'static str {
        "OpenClaw"
    }

    fn discover(&self) -> DiscoveryResult {
        let detected = Self::is_detected();
        let detected_path = if detected {
            Self::openclaw_dir()
        } else {
            None
        };

        DiscoveryResult {
            id: self.id(),
            display_name: self.display_name(),
            detected,
            detected_path,
            detection_hint: Some(
                "Install OpenClaw: npm install -g openclaw or visit https://openclaw.ai".into(),
            ),
            hooks_installed: false, // Will be set by discover_with_hooks_check
        }
    }

    fn supported_hooks(&self) -> &'static [&'static str] {
        OPENCLAW_HOOKS
    }
}

impl ProviderFactory for OpenClawDiscovery {
    fn create(&self) -> Result<Box<dyn LLmProviderTrait>> {
        Ok(Box::new(OpenClaw::new()?))
    }
}

pub struct OpenClaw {
    /// Path to the viberails binary
    binary_path: PathBuf,
}

impl OpenClaw {
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

    /// Get the plugin directory path for the `OpenClaw` installation.
    fn plugin_dir(install_dir: &std::path::Path) -> PathBuf {
        install_dir.join("extensions").join(PROJECT_NAME)
    }

    /// Generate the plugin manifest JSON content.
    /// Ref: <https://docs.openclaw.ai/plugin> for manifest format.
    pub(crate) fn generate_plugin_manifest() -> String {
        serde_json::to_string_pretty(&json!({
            "id": PROJECT_NAME,
            "name": format!("{} Security Plugin", PROJECT_NAME),
            "version": "1.0.0",
            "description": format!("{} security and compliance monitoring for tool calls", PROJECT_NAME),
            "main": "index.ts",
            "configSchema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }
        }))
        .unwrap_or_default()
    }

    /// Escape a string for safe embedding in JavaScript code.
    /// Prevents injection attacks when embedding paths in generated TypeScript.
    fn escape_js_string(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('$', "\\$") // Prevent template literal injection
            .replace('`', "\\`") // Prevent backtick template strings
    }

    /// Generate the plugin index.ts content that registers the `before_tool_call` hook.
    /// This uses the `OpenClaw` plugin API to register lifecycle hooks that intercept tool calls
    /// before they are executed.
    /// Ref: <https://docs.openclaw.ai/plugin>
    pub(crate) fn generate_plugin_index(&self) -> String {
        let binary_path = Self::escape_js_string(&self.binary_path.display().to_string());
        format!(
            r#"import {{ spawnSync }} from "child_process";

/**
 * {PROJECT_NAME} Plugin for OpenClaw
 *
 * This plugin intercepts tool calls via the before_tool_call lifecycle hook
 * for security and compliance monitoring.
 * Ref: https://docs.openclaw.ai/plugin
 */

/**
 * OpenClaw before_tool_call event format
 */
interface BeforeToolCallEvent {{
  toolName: string;
  params: Record<string, unknown>;
}}

/**
 * OpenClaw context passed to lifecycle hooks
 */
interface HookContext {{
  agentId?: string;
  sessionKey?: string;
  toolName: string;
}}

/**
 * Response format for before_tool_call hook
 * Return {{ block: true, blockReason: "..." }} to block the tool call
 * Return {{ params: modifiedParams }} to modify parameters
 * Return undefined or void to allow
 */
interface BeforeToolCallResponse {{
  block?: boolean;
  blockReason?: string;
  params?: Record<string, unknown>;
}}

function callViberails(
  event: BeforeToolCallEvent,
  ctx: HookContext
): BeforeToolCallResponse | undefined {{
  const eventJson = JSON.stringify({{ ...event, ...ctx }});

  try {{
    const result = spawnSync("{binary_path}", ["openclaw-callback"], {{
      input: eventJson,
      encoding: "utf-8",
      timeout: 30000, // 30 second timeout
    }});

    if (result.error) {{
      console.error(`[{PROJECT_NAME}] Spawn error: ${{result.error.message}}`);
      return undefined; // Fail open on spawn errors
    }}

    if (result.status !== 0) {{
      console.error(`[{PROJECT_NAME}] Process exited with code ${{result.status}}: ${{result.stderr}}`);
      return undefined; // Fail open on process errors
    }}

    if (result.stdout?.trim()) {{
      try {{
        const response = JSON.parse(result.stdout.trim());
        if (response.decision === "block") {{
          return {{
            block: true,
            blockReason: response.reason || "Blocked by {PROJECT_NAME} policy",
          }};
        }}
      }} catch {{
        // Ignore parse errors, response may be empty for allowed actions
      }}
    }}

    return undefined; // Allow the tool call
  }} catch {{
    console.error(`[{PROJECT_NAME}] Exception during tool call interception`);
    return undefined; // Fail open on exceptions
  }}
}}

/**
 * OpenClaw Plugin using lifecycle hooks (api.on)
 */
export default {{
  id: "{PROJECT_NAME}",
  name: "{PROJECT_NAME} Security Plugin",
  description: "Security and compliance monitoring for tool calls",
  register(api: {{ on: (hook: string, handler: (event: BeforeToolCallEvent, ctx: HookContext) => BeforeToolCallResponse | void) => void }}) {{
    // Register before_tool_call lifecycle hook to intercept tool executions
    // This fires before any tool is executed
    api.on("before_tool_call", (event, ctx) => {{
      const {{ toolName, params }} = event;

      // Skip if no tool name
      if (!toolName) {{
        return;
      }}

      const result = callViberails({{ toolName, params }}, ctx);

      // Return the blocking response or undefined to allow
      return result;
    }});

    console.log(`[{PROJECT_NAME}] Plugin loaded - monitoring tool calls via before_tool_call hook`);
  }}
}};
"#
        )
    }

    /// Install plugin files into the `OpenClaw` extensions directory.
    fn install_plugin_files(&self, install_dir: &std::path::Path) -> Result<()> {
        let plugin_dir = Self::plugin_dir(install_dir);

        // Create plugin directory
        fs::create_dir_all(&plugin_dir).with_context(|| {
            format!(
                "Unable to create plugin directory at {}",
                plugin_dir.display()
            )
        })?;

        // Write plugin manifest
        let manifest_path = plugin_dir.join("openclaw.plugin.json");
        fs::write(&manifest_path, Self::generate_plugin_manifest()).with_context(|| {
            format!(
                "Unable to write openclaw.plugin.json at {}",
                manifest_path.display()
            )
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

    /// Remove plugin files from the extensions directory.
    fn uninstall_plugin_files(install_dir: &std::path::Path) -> Result<()> {
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

        Ok(())
    }

    /// Disable the plugin in the config file.
    fn disable_in_config(config_file: &std::path::Path) -> Result<()> {
        if !config_file.exists() {
            return Ok(());
        }

        let data = fs::read_to_string(config_file)
            .with_context(|| format!("Unable to read {}", config_file.display()))?;

        let mut json: Value = serde_json::from_str(&data)
            .with_context(|| format!("Unable to parse JSON in {}", config_file.display()))?;

        // Remove from plugins.entries
        let removed = json
            .as_object_mut()
            .and_then(|root| root.get_mut("plugins"))
            .and_then(|p| p.as_object_mut())
            .and_then(|p| p.get_mut("entries"))
            .and_then(|e| e.as_object_mut())
            .and_then(|entries| entries.remove(PROJECT_NAME));

        if removed.is_none() {
            warn!("{PROJECT_NAME} not found in {}", config_file.display());
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
        let root = json
            .as_object_mut()
            .ok_or_else(|| anyhow!("Expected root to be a JSON object"))?;

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
        let _ = json
            .as_object_mut()
            .and_then(|root| root.get_mut("plugins"))
            .and_then(|p| p.as_object_mut())
            .and_then(|p| p.get_mut("entries"))
            .and_then(|e| e.as_object_mut())
            .and_then(|entries| entries.remove(PROJECT_NAME));
    }
}

impl LLmProviderTrait for OpenClaw {
    fn name(&self) -> &'static str {
        "openclaw"
    }

    fn install(&self, hook_type: &str) -> Result<()> {
        let Some((install_dir, config_name)) = OpenClawDiscovery::openclaw_paths() else {
            anyhow::bail!("No OpenClaw installation detected");
        };

        if !install_dir.exists() {
            anyhow::bail!("OpenClaw directory not found at {}", install_dir.display());
        }

        info!(
            "Installing {hook_type} plugin in {}",
            install_dir.display()
        );

        // Install plugin files (manifest and index.ts)
        self.install_plugin_files(&install_dir)?;

        // Enable in config
        let config_file = install_dir.join(config_name);
        Self::enable_in_config(&config_file)?;

        Ok(())
    }

    fn uninstall(&self, hook_type: &str) -> Result<()> {
        let Some((install_dir, config_name)) = OpenClawDiscovery::openclaw_paths() else {
            return Ok(());
        };

        if !install_dir.exists() {
            return Ok(());
        }

        info!(
            "Uninstalling {hook_type} from {}",
            install_dir.display()
        );

        // Remove plugin files
        Self::uninstall_plugin_files(&install_dir)?;

        // Disable in config
        let config_file = install_dir.join(config_name);
        Self::disable_in_config(&config_file)?;

        Ok(())
    }

    fn list(&self) -> Result<Vec<HookEntry>> {
        let mut entries = Vec::new();

        let Some((install_dir, config_name)) = OpenClawDiscovery::openclaw_paths() else {
            return Ok(entries);
        };

        let config_file = install_dir.join(config_name);

        if !config_file.exists() {
            return Ok(entries);
        }

        let data = fs::read_to_string(&config_file)
            .with_context(|| format!("Unable to read {}", config_file.display()))?;

        let json: Value = serde_json::from_str(&data)
            .with_context(|| format!("Unable to parse JSON in {}", config_file.display()))?;

        // Check plugins.entries
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

        Ok(entries)
    }
}
