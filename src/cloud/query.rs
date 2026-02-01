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
    secret: String,
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

        let username = whoami::username().ok();

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

        // Parse the URL and extract the secret from the last path segment
        // URL format: https://{hooks_domain}/{oid}/{adapter_name}/{secret}
        let (url, secret) = Self::extract_secret_from_url(&config.org.url)?;

        info!("Using url={url}");

        Ok(Self {
            config,
            url,
            secret,
            provider,
        })
    }

    /// Extract the secret from the webhook URL and return the URL without it.
    /// The secret is sent via header to avoid proxies logging it in access logs.
    fn extract_secret_from_url(full_url: &str) -> Result<(String, String)> {
        let mut parsed =
            url::Url::parse(full_url).context("Invalid webhook URL format")?;

        // Get path segments and extract the last one as the secret
        let segments: Vec<&str> = parsed
            .path_segments()
            .context("Webhook URL has no path segments")?
            .collect();

        if segments.len() < 3 {
            bail!(
                "Invalid webhook URL format. Expected: https://hooks.domain/oid/name/secret"
            );
        }

        // The last segment is the secret
        let secret = segments
            .last()
            .context("No secret segment in URL")?
            .to_string();

        if secret.is_empty() {
            bail!("Secret segment in webhook URL cannot be empty");
        }

        // Rebuild the path without the secret (we know segments.len() >= 3)
        let path_without_secret: String = segments
            .get(..segments.len().saturating_sub(1))
            .unwrap_or(&[])
            .join("/");
        parsed.set_path(&format!("/{path_without_secret}"));

        Ok((parsed.to_string(), secret))
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
            .with_header("lc-secret", &self.secret)
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
            .with_header("lc-secret", &self.secret)
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
