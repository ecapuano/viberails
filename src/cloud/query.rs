use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use derive_more::Display;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    cloud::REQUEST_TIMEOUT_SECS, common::display_authorize_help, config::Config,
    providers::Providers,
};

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CloudQueryType {
    Auth,
    Notify,
}

#[derive(Display)]
pub enum CloudVerdict {
    Allow,
    Deny(String),
}

#[derive(Deserialize)]
struct CloudResponse {
    success: bool,
    reason: Option<String>,
    //error: Option<String>,
    //rejected: Option<String>,
    //rule: Option<String>,
}

#[derive(Serialize)]
struct CloudRequestMeta<'a> {
    ts: u128,
    installation_id: &'a str,
    request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    source: &'a Providers,
    #[serde(rename = "type")]
    query_type: CloudQueryType,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
}

#[derive(Serialize)]
struct CloudRequest<'a> {
    meta_data: CloudRequestMeta<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notify: Option<Value>,
}

pub struct CloudQuery<'a> {
    config: &'a Config,
    url: String,
    provider: Providers,
}

fn mine_session_id(data: &Value) -> Option<String> {
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
    pub fn new(
        config: &'a Config,
        session_id: Option<String>,
        source: &'a Providers,
        query_type: CloudQueryType,
    ) -> Result<Self> {
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

        let username = if let Ok(username) = whoami::username() {
            Some(username)
        } else {
            None
        };

        Ok(Self {
            ts,
            installation_id,
            request_id,
            hostname,
            session_id,
            source,
            query_type,
            username,
        })
    }
}

impl<'a> CloudQuery<'a> {
    pub fn new(config: &'a Config, provider: Providers) -> Result<Self> {
        //
        // bail if we're not actually yet authorized
        //
        if !config.org.authorized() {
            display_authorize_help();
            bail!("Not yet authorized")
        }

        info!("Authorized for oid={}", config.org.oid);

        let url = config.org.url.clone();

        info!("Using url={url}");

        Ok(Self {
            config,
            url,
            provider,
        })
    }

    pub fn notify(&self, data: Value) -> Result<()> {
        let session_id = mine_session_id(&data);

        let meta_data = CloudRequestMeta::new(
            self.config,
            session_id,
            &self.provider,
            CloudQueryType::Notify,
        )?;
        let req = CloudRequest {
            meta_data,
            notify: Some(data),
            auth: None,
        };

        let ret = minreq::post(&self.url)
            .with_timeout(REQUEST_TIMEOUT_SECS)
            .with_json(&req)
            .context("Failed to serialize notification request")?
            .send();

        if let Err(e) = ret {
            error!("Notification to {} failed: {e}", self.url);
        }

        info!("successfully sent the notification");

        Ok(())
    }

    pub fn authorize(&self, data: Value) -> Result<CloudVerdict> {
        let session_id = mine_session_id(&data);

        let meta_data = CloudRequestMeta::new(
            self.config,
            session_id,
            &self.provider,
            CloudQueryType::Auth,
        )?;

        let req = CloudRequest {
            meta_data,
            auth: Some(data),
            notify: None,
        };

        let res = minreq::post(&self.url)
            .with_timeout(REQUEST_TIMEOUT_SECS)
            .with_json(&req)
            .context("Failed to serialize authorization request")?
            .send()
            .with_context(|| format!("Failed to connect to hook server at {}", self.url))?;

        let data = res.as_str()?;

        info!("cloud returned {data}");

        let data: CloudResponse = res
            .json()
            .context("Authorization server returned invalid JSON response")?;

        info!("allow={} reason={:?}", data.success, data.reason);

        let verdict = if data.success {
            CloudVerdict::Allow
        } else {
            let msg = if let Some(reason) = data.reason {
                format!("deny reason: {reason}")
            } else {
                String::new()
            };

            CloudVerdict::Deny(msg)
        };

        Ok(verdict)
    }
}
