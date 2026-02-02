use anyhow::{Context, Result, bail};
use log::{error, info};

use crate::{
    cloud::query::CloudQuery,
    common::PROJECT_NAME,
    config::Config,
    providers::{
        LLmProviderTrait, Providers, claude::Claude, codex::Codex, cursor::Cursor, gemini::Gemini,
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
        Providers::Codex => Codex::new()?.io(&cloud, &config),
        Providers::OpenCode => OpenCode::new()?.io(&cloud, &config),
        Providers::OpenClaw => OpenClaw::new()?.io(&cloud, &config),
    }
}
