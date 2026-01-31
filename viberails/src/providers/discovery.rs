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
    /// Create a new provider instance using the given program path.
    /// The program path is the path to the viberails binary that will be used in hook commands.
    fn create(&self, program_path: &std::path::Path) -> Result<Box<dyn LLmProviderTrait>>;
}
