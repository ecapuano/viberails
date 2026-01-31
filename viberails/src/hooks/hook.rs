use std::{
    io::{BufRead, BufReader, BufWriter, Stdin, Stdout, Write, stdin, stdout},
    time::Instant,
};

use anyhow::{Context, Result, bail};
use derive_more::Display;
use log::{error, info, warn};
use serde::Serialize;
use serde_json::Value;

use crate::{
    cloud::query::{CloudQuery, CloudVerdict},
    common::PROJECT_NAME,
    config::Config,
    providers::Providers,
};

const TOOL_HINTS: &[&str] = &["tool_input", "tool_name", "tool_use_id"];

#[derive(Serialize, Display)]
#[allow(dead_code)]
#[serde(rename_all = "lowercase")]
enum HookDecision {
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

fn is_tool_use(value: &Value) -> bool {
    for hint in TOOL_HINTS {
        if value.get(hint).is_some() {
            return true;
        }
    }

    false
}

struct Hook<'a> {
    config: &'a Config,
    cloud: CloudQuery<'a>,
    reader: BufReader<Stdin>,
    writer: BufWriter<Stdout>,
}

impl<'a> Hook<'a> {
    pub fn new(config: &'a Config, provider: Providers) -> Result<Self> {
        //
        // This'll fail if we're not authorized
        //
        let cloud = CloudQuery::new(config, provider).context("Unable to initialize Cloud API")?;

        let stdin = stdin();
        let stdout = stdout();

        let reader = BufReader::new(stdin);
        let writer = BufWriter::new(stdout);

        Ok(Self {
            config,
            cloud,
            reader,
            writer,
        })
    }

    fn authorize_tool(&self, value: Value) -> HookDecision {
        //
        // Do we fail-open?
        //
        match self.cloud.authorize(value) {
            Ok(CloudVerdict::Allow) => HookDecision::Approve,
            Ok(CloudVerdict::Deny(r)) => {
                warn!("Deny reason: {r}");
                HookDecision::Block(r)
            }
            Err(e) => {
                error!("cloud failed ({e})");

                if self.config.user.fail_open {
                    HookDecision::Approve
                } else {
                    let msg = format!("{PROJECT_NAME} cloud failure ({e})");
                    HookDecision::Block(msg)
                }
            }
        }
    }

    fn write_decision(&mut self, decision: HookDecision) -> Result<()> {
        let answer: HookAnswer = decision.into();

        let resp_string =
            serde_json::to_string(&answer).context("Failed to serialize hook response")?;

        info!("decision json: {resp_string}");

        self.writer
            .write_all(resp_string.as_bytes())
            .context("Failed to write hook response to stdout")?;
        self.writer
            .flush()
            .context("Failed to flush hook response")?;

        Ok(())
    }

    pub fn wait_for_input(&mut self) -> Result<()> {
        let mut line = String::new();

        info!("Wating for input");

        // that's a fatal error
        let len = self
            .reader
            .read_line(&mut line)
            .context("Unable to read from stdin")?;

        if 0 == len {
            // that's still successful, out input just got closed
            warn!("EOF. We're leaving");
            return Ok(());
        }

        let value = serde_json::from_str(&line).context("Unable to deserialize")?;

        let start = Instant::now();

        if is_tool_use(&value) {
            //
            // D&R Path - only call cloud if audit_tool_use is enabled
            //
            let decision = if self.config.user.audit_tool_use {
                self.authorize_tool(value)
            } else {
                info!("audit_tool_use disabled, approving locally");
                HookDecision::Approve
            };

            info!("Decision={decision}");
            self.write_decision(decision)?;
        } else {
            //
            // Notify path - only call cloud if audit_prompts is enabled
            //
            if self.config.user.audit_prompts {
                if let Err(e) = self.cloud.notify(value) {
                    error!("Unable to notify cloud ({e})");
                }
            } else {
                info!("audit_prompts disabled, skipping cloud notification");
            }
        }

        let duration = start.elapsed().as_millis();

        info!("duration={duration}ms");

        Ok(())
    }
}

pub fn hook(provider: Providers) -> Result<()> {
    info!("{PROJECT_NAME} is starting");

    let config = Config::load()?;

    if !config.org.authorized() {
        bail!("not authorized");
    }

    let mut hook = Hook::new(&config, provider)?;

    hook.wait_for_input()
}
