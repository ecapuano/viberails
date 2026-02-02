use std::time::Instant;

use anyhow::{Context, Result, bail};
use log::{error, info};

use crate::{
    cloud::query::CloudQuery,
    common::PROJECT_NAME,
    config::Config,
    providers::{
        LLmProviderTrait, Providers, claude::Claude, cursor::Cursor, gemini::Gemini,
        openclaw::OpenClaw, opencode::OpenCode,
    },
};

pub fn hook(provider: Providers) -> Result<()> {
    info!("{PROJECT_NAME} is starting");

    let config = Config::load()?;

    if !config.org.authorized() {
        bail!("not authorized");
    }

    //
    // This'll fail if we're not authorized
    //
    let ret = CloudQuery::new(&config, provider).context("Unable to initialize Cloud API");

    //
    // Let the user decide to fail open if not properly configured
    //
    let cloud = match ret {
        Ok(v) => v,
        Err(e) => {
            error!("Unable to init cloud {e}");
            if config.user.fail_open {
                return Ok(());
            }
            return Err(e);
        }
    };

    match provider {
        Providers::ClaudeCode => Claude::new()?.io(&cloud, &config),
        Providers::Cursor => Cursor::new()?.io(&cloud, &config),
        Providers::GeminiCli => Gemini::new()?.io(&cloud, &config),
        Providers::OpenCode => OpenCode::new()?.io(&cloud, &config),
        Providers::OpenClaw => OpenClaw::new()?.io(&cloud, &config),
        Providers::Codex => bail!("Codex requires payload argument, use codex_hook() instead"),
    }
}

/// Codex-specific hook that receives JSON payload as a command line argument
/// (unlike other providers that read from stdin)
pub fn codex_hook(payload: &str) -> Result<()> {
    info!("{PROJECT_NAME} is starting (codex)");

    let config = Config::load()?;

    if !config.org.authorized() {
        bail!("not authorized");
    }

    let ret = CloudQuery::new(&config, Providers::Codex).context("Unable to initialize Cloud API");

    let cloud = match ret {
        Ok(v) => v,
        Err(e) => {
            error!("Unable to init cloud {e}");
            if config.user.fail_open {
                return Ok(());
            }
            return Err(e);
        }
    };

    info!("Received JSON payload (length={})", payload.len());

    let value: serde_json::Value =
        serde_json::from_str(payload).context("Unable to deserialize JSON payload")?;

    let start = Instant::now();

    // Codex notify is for notifications only (e.g., agent-turn-complete)
    // It doesn't require a response, so we just send to cloud if audit_prompts is enabled
    if config.user.audit_prompts {
        if let Err(e) = cloud.notify(value) {
            error!("Unable to notify cloud ({e})");
        }
    } else {
        info!("audit_prompts disabled, skipping cloud notification");
    }

    let duration = start.elapsed().as_millis();
    info!("duration={duration}ms");

    Ok(())
}
