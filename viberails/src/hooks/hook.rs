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

struct Hook<'a> {
    config: &'a Config,
    cloud: CloudQuery<'a>,
}

impl<'a> Hook<'a> {
    pub fn new(config: &'a Config, provider: Providers) -> Result<Self> {
        //
        // This'll fail if we're not authorized
        //
        let cloud = CloudQuery::new(config, provider).context("Unable to initialize Cloud API")?;

        Ok(Self { config, cloud })
    }

    pub fn io<H>(&self, handler: &H) -> Result<()>
    where
        H: LLmProviderTrait,
    {
        handler.io(&self.cloud, self.config)
    }
}

pub fn hook(provider: Providers) -> Result<()> {
    info!("{PROJECT_NAME} is starting");

    let config = Config::load()?;

    if !config.org.authorized() {
        bail!("not authorized");
    }

    let hook = Hook::new(&config, provider)?;

    match provider {
        Providers::ClaudeCode => hook.io(&Claude::new()?),
        Providers::Cursor => hook.io(&Cursor::new()?),
        Providers::GeminiCli => hook.io(&Gemini::new()?),
        Providers::Codex => hook.io(&Codex::new()?),
        Providers::OpenCode => hook.io(&OpenCode::new()?),
        Providers::Clawdbot => hook.io(&Clawdbot::new()?),
    }
}
