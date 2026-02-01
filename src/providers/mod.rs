use std::{
    io::{BufRead, BufReader, BufWriter, Stdout, Write},
    time::Instant,
};

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
use log::{error, info, warn};
pub use registry::ProviderRegistry;
pub use selector::{select_providers, select_providers_for_uninstall};

use crate::{
    cloud::query::{CloudQuery, CloudVerdict},
    common::PROJECT_NAME,
    config::Config,
};
use anyhow::Context;
use derive_more::Display;
use serde::Serialize;
use serde_json::Value;

#[cfg(test)]
mod tests;

const TOOL_HINTS: &[&str] = &["tool_input", "tool_name", "tool_use_id"];

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

#[derive(Serialize, Display)]
#[allow(dead_code)]
#[serde(rename_all = "lowercase")]
pub enum HookDecision {
    Block(String),
    Approve,
}

#[derive(Serialize)]
struct HookAnswer {
    decision: HookDecision,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl From<HookDecision> for HookAnswer {
    fn from(value: HookDecision) -> Self {
        match value {
            HookDecision::Block(ref r) => {
                let reason = r.clone();
                Self {
                    decision: value,
                    reason: Some(reason),
                }
            }
            HookDecision::Approve => Self {
                decision: value,
                reason: None,
            },
        }
    }
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

    fn write_decision(&self, writer: &mut BufWriter<Stdout>, decision: HookDecision) -> Result<()> {
        let answer: HookAnswer = decision.into();

        let resp_string =
            serde_json::to_string(&answer).context("Failed to serialize hook response")?;

        info!("decision json: {resp_string}");

        writer
            .write_all(resp_string.as_bytes())
            .context("Failed to write hook response to stdout")?;
        writer.flush().context("Failed to flush hook response")?;

        Ok(())
    }

    fn authorize_tool(&self, cloud: &CloudQuery, config: &Config, value: Value) -> HookDecision {
        //
        // Do we fail-open?
        //
        match cloud.authorize(value) {
            Ok(CloudVerdict::Allow) => HookDecision::Approve,
            Ok(CloudVerdict::Deny(r)) => {
                warn!("Deny reason: {r}");
                HookDecision::Block(r)
            }
            Err(e) => {
                error!("cloud failed ({e})");

                if config.user.fail_open {
                    HookDecision::Approve
                } else {
                    let msg = format!("{PROJECT_NAME} cloud failure ({e})");
                    HookDecision::Block(msg)
                }
            }
        }
    }

    fn is_tool_use(&self, value: &Value) -> bool {
        for hint in TOOL_HINTS {
            if value.get(hint).is_some() {
                return true;
            }
        }

        false
    }
    fn io(&self, cloud: &CloudQuery, config: &Config) -> Result<()> {
        //
        // This'll fail if we're not authorized
        //

        let stdin = std::io::stdin();
        let stdout = std::io::stdout();

        let mut reader = BufReader::new(stdin);
        let mut writer = BufWriter::new(stdout);

        let mut line = String::new();

        info!("Waiting for input");

        // that's a fatal error
        let len = reader
            .read_line(&mut line)
            .context("Unable to read from stdin")?;

        if 0 == len {
            // that's still successful, out input just got closed
            warn!("EOF. We're leaving");
            return Ok(());
        }

        let value = serde_json::from_str(&line).context("Unable to deserialize")?;

        let start = Instant::now();

        if self.is_tool_use(&value) {
            //
            // D&R Path - only call cloud if audit_tool_use is enabled
            //
            let decision = if config.user.audit_tool_use {
                self.authorize_tool(cloud, config, value)
            } else {
                info!("audit_tool_use disabled, approving locally");
                HookDecision::Approve
            };

            info!("Decision={decision}");
            self.write_decision(&mut writer, decision)?;
        } else {
            //
            // Notify path - only call cloud if audit_prompts is enabled
            //
            if config.user.audit_prompts {
                if let Err(e) = cloud.notify(value) {
                    error!("Unable to notify cloud ({e})");
                }
            } else {
                info!("audit_prompts disabled, skipping cloud notification");
            }

            // Always write an approve response for non-tool-use events
            // The AI tool may be waiting for a response on stdout
            self.write_decision(&mut writer, HookDecision::Approve)?;
        }

        let duration = start.elapsed().as_millis();

        info!("duration={duration}ms");

        Ok(())
    }
}
