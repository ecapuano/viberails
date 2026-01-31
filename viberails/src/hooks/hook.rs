use anyhow::{Context, Result, bail};
use log::info;

use crate::{
    cloud::query::CloudQuery,
    common::PROJECT_NAME,
    config::Config,
    providers::{
        LLmProviderTrait, Providers, claude::Claude, clawdbot::Clawdbot, codex::Codex,
        cursor::Cursor, gemini::Gemini, opencode::OpenCode,
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
    let cloud = CloudQuery::new(&config, provider).context("Unable to initialize Cloud API")?;

    match provider {
        Providers::ClaudeCode => Claude::new()?.io(&cloud, &config),
        Providers::Cursor => Cursor::new()?.io(&cloud, &config),
        Providers::GeminiCli => Gemini::new()?.io(&cloud, &config),
        Providers::Codex => Codex::new()?.io(&cloud, &config),
        Providers::OpenCode => OpenCode::new()?.io(&cloud, &config),
        Providers::Clawdbot => Clawdbot::new()?.io(&cloud, &config),
    }
}
