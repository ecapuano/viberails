use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use derive_more::Display;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{cloud::REQUEST_TIMEOUT_SECS, config::Config};

#[derive(Display)]
pub enum CloudVerdict {
    Allow,
    Deny(String),
}

#[derive(Deserialize)]
struct CloudResponse {
    allow: bool,
    reason: String,
}

#[derive(Serialize)]
struct CloudRequestMeta<'a> {
    ts: u128,
    installation_id: &'a str,
    request_id: String,
    hostname: Option<String>,
}

#[derive(Serialize)]
struct CloudRequestAuth<'a> {
    meta_data: CloudRequestMeta<'a>,
    hook_data: Value,
    session_id: Option<String>,
}

#[derive(Serialize)]
struct CloudRequestNotify<'a> {
    meta_data: CloudRequestMeta<'a>,
    hook_data: Value,
    session_id: Option<String>,
}

#[derive(Serialize, Default)]
struct CloudRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<CloudRequestAuth<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notify: Option<CloudRequestNotify<'a>>,
}

fn find_session_id(data: &Value) -> Option<String> {
    //
    // This is to be accomodating for various providers and or versions
    // so we're mining for some kind of session id
    //
    if let Some(session_value) = data.get("session_id")
        && let Some(session_id) = session_value.as_str()
    {
        return Some(session_id.to_string());
    }

    //
    // We'll log it and hopefully it'll percolate so we can fix this
    //
    warn!("Unable to find a session id in hook data");
    None
}

impl<'a> CloudRequestMeta<'a> {
    pub fn new(config: &'a Config) -> Result<Self> {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .context("Unable to get current timestamp")?
            .as_millis();

        let installation_id = config.install_id.as_str();
        let request_id = Uuid::new_v4().to_string();

        let hostname = if let Ok(host) = hostname::get() {
            if let Ok(host) = host.into_string() {
                Some(host)
            } else {
                warn!("Unable to get localhostname");
                None
            }
        } else {
            warn!("Unable to get localhostname");
            None
        };

        Ok(Self {
            ts,
            installation_id,
            request_id,
            hostname,
        })
    }
}

impl<'a> CloudRequestNotify<'a> {
    pub fn new(config: &'a Config, hook_data: Value) -> Result<Self> {
        let session_id = find_session_id(&hook_data);

        let meta_data = CloudRequestMeta::new(config)?;

        Ok(Self {
            meta_data,
            hook_data,
            session_id,
        })
    }
}

impl<'a> CloudRequestAuth<'a> {
    pub fn new(config: &'a Config, hook_data: Value) -> Result<Self> {
        let session_id = find_session_id(&hook_data);

        let meta_data = CloudRequestMeta::new(config)?;

        Ok(Self {
            meta_data,
            hook_data,
            session_id,
        })
    }
}

pub struct CloudQuery<'a> {
    config: &'a Config,
    bearer: String,
    url: String,
}

impl<'a> CloudQuery<'a> {
    pub fn new(config: &'a Config) -> Result<Self> {
        //
        // bail if we're not actually yet authorized
        //
        if !config.org.authorized() {
            bail!("Not yet authorized")
        }

        info!("Authorized for oid={}", config.org.oid);

        let bearer = format!("Bearer {}", config.org.jwt);

        let url = format!("{}/{}/test-dr", config.user.hook_url, config.org.oid);

        info!("Using url={}", url);

        Ok(Self {
            config,
            bearer,
            url,
        })
    }

    pub fn notify(&self, data: Value) -> Result<()> {
        let notify = CloudRequestNotify::new(self.config, data)?;
        let req = CloudRequest {
            notify: Some(notify),
            ..Default::default()
        };

        let ret = minreq::post(&self.url)
            .with_timeout(REQUEST_TIMEOUT_SECS)
            .with_header("Authorization", &self.bearer)
            .with_json(&req)
            .context("Failed to serialize notification request")?
            .send();

        if let Err(e) = ret {
            error!("Notification to {} failed: {e}", self.config.user.hook_url);
        }

        Ok(())
    }

    pub fn authorize(&self, data: Value) -> Result<CloudVerdict> {
        let auth = CloudRequestAuth::new(self.config, data)?;

        let req = CloudRequest {
            auth: Some(auth),
            ..Default::default()
        };

        let res = minreq::post(&self.url)
            .with_timeout(REQUEST_TIMEOUT_SECS)
            .with_header("Authorization", &self.bearer)
            .with_json(&req)
            .context("Failed to serialize authorization request")?
            .send()
            .with_context(|| {
                format!(
                    "Failed to connect to hook server at {}",
                    self.config.user.hook_url
                )
            })?;

        let data: CloudResponse = res
            .json()
            .context("Authorization server returned invalid JSON response")?;

        let verdict = if data.allow {
            CloudVerdict::Allow
        } else {
            CloudVerdict::Deny(data.reason)
        };

        Ok(verdict)
    }
}
