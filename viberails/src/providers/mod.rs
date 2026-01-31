use anyhow::Result;

pub mod claude;
pub mod clawdbot;
pub mod codex;
pub mod cursor;
pub mod discovery;
pub mod gemini;
pub mod opencode;
pub mod registry;
pub mod selector;

// Re-exports for public API (may be used by external code or future extensions)
#[allow(unused_imports)]
pub use discovery::{DiscoveryResult, ProviderDiscovery, ProviderFactory};
pub use registry::ProviderRegistry;
pub use selector::{select_providers, select_providers_for_uninstall};

use derive_more::Display;
use serde::Serialize;

#[cfg(test)]
mod tests;

/// Enum representing all supported providers.
/// Used for callback command routing.
#[derive(Clone, Copy, clap::ValueEnum, Display, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Providers {
    ClaudeCode,
    Cursor,
    GeminiCli,
    Codex,
    OpenCode,
    Clawdbot,
}

pub struct HookEntry {
    pub hook_type: String,
    pub matcher: String,
    pub command: String,
}

pub trait LLmProviderTrait {
    #[allow(dead_code)]
    fn name(&self) -> &'static str;
    fn install(&self, hook_type: &str) -> Result<()>;
    fn uninstall(&self, hook_type: &str) -> Result<()>;
    fn list(&self) -> Result<Vec<HookEntry>>;
}
