use std::path::PathBuf;

use anyhow::Result;

use super::LLmProviderTrait;

/// Result of checking whether a provider tool is available on the system.
#[derive(Debug, Clone)]
pub struct DiscoveryResult {
    /// The provider's unique identifier (e.g., "claude-code")
    pub id: &'static str,
    /// Human-readable name (e.g., "Claude Code")
    pub display_name: &'static str,
    /// Whether the tool was detected on the system
    pub detected: bool,
    /// Path where the tool was detected (if found)
    #[allow(dead_code)]
    pub detected_path: Option<PathBuf>,
    /// Hint for users when tool is not detected (e.g., "Install Claude Code from...")
    pub detection_hint: Option<String>,
    /// Whether our hooks are installed in this tool (only meaningful if detected is true)
    pub hooks_installed: bool,
}

/// Trait for discovering whether a provider tool is installed.
/// Discovery operations are read-only and should have no side effects.
pub trait ProviderDiscovery: Send + Sync {
    /// Unique identifier for this provider (e.g., "claude-code")
    fn id(&self) -> &'static str;

    /// Human-readable display name (e.g., "Claude Code")
    fn display_name(&self) -> &'static str;

    /// Check if the tool is installed on the system.
    /// This should be a read-only operation with no side effects.
    fn discover(&self) -> DiscoveryResult;

    /// List of hook types this provider supports (e.g., `PreToolUse`, `UserPromptSubmit`)
    fn supported_hooks(&self) -> &'static [&'static str];
}

/// Trait for creating provider instances after discovery confirms availability.
pub trait ProviderFactory: ProviderDiscovery {
    /// Create a new provider instance.
    /// The provider will use `std::env::current_exe()` to determine the program path for hook commands.
    fn create(&self) -> Result<Box<dyn LLmProviderTrait>>;

    /// Discover the provider and check if our hooks are installed.
    /// This calls `discover()` first, then if the tool is detected, creates the provider
    /// and checks if any of its hooks contain our command.
    fn discover_with_hooks_check(&self) -> DiscoveryResult {
        let mut result = self.discover();

        if !result.detected {
            return result;
        }

        // Try to create provider and check for our hooks
        if let Ok(provider) = self.create() {
            if let Ok(hooks) = provider.list() {
                // Check if any hook command contains our binary name
                // We look for the callback command pattern (e.g., "viberails claude-callback")
                result.hooks_installed = hooks.iter().any(|h| {
                    h.command.contains("-callback") && h.command.contains("viberails")
                        || h.command.ends_with("/viberails")
                        || h.command.contains("/viberails ")
                });
            }
        }

        result
    }
}
