use anyhow::{Context, Result};
use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::{cloud::REQUEST_TIMEOUT_SECS, common::PROJECT_NAME};

const LC_JWT_URL: &str = "https://jwt.limacharlie.io";
const LC_API_URL: &str = "https://api.limacharlie.io/v1";

#[derive(Deserialize)]
struct LcJwtResponse {
    jwt: String,
}

#[derive(Deserialize)]
struct LcOrgAvailable {
    is_available: bool,
}

#[derive(Deserialize)]
struct OrgCreateResponse {
    data: OrgCreateData,
}

#[derive(Deserialize)]
struct OrgCreateData {
    oid: String,
}

#[derive(Debug, Deserialize)]
struct OrgUrlsResponse {
    url: OrgUrls,
}

#[derive(Debug, Deserialize)]
pub struct OrgUrls {
    pub hooks: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InstallationKeyResponse {
    iid: String,
}

#[derive(Serialize)]
struct WebhookAdapterData<'a> {
    sensor_type: &'a str,
    webhook: WebhookConfig<'a>,
}

#[derive(Serialize)]
struct WebhookConfig<'a> {
    secret: &'a str,
    client_options: ClientOptions<'a>,
}

#[derive(Serialize)]
struct ClientOptions<'a> {
    hostname: &'a str,
    identity: Identity<'a>,
    platform: &'a str,
    sensor_seed_key: &'a str,
    mapping: Mapping<'a>,
}

#[derive(Serialize)]
struct Mapping<'a> {
    event_type_path: &'a str,
    sensor_key_path: &'a str,
    sensor_hostname_path: &'a str,
}

#[derive(Serialize)]
struct Identity<'a> {
    oid: &'a str,
    installation_key: &'a str,
}

#[derive(Builder)]
pub struct WebhookAdapter<'a> {
    token: &'a str,
    oid: &'a str,
    name: &'a str,
    secret: &'a str,
    installation_key: &'a str,
    sensor_seed_key: &'a str,
}

pub fn get_jwt_firebase<S, K>(oid: S, fb_auth: K) -> Result<String>
where
    S: AsRef<str>,
    K: AsRef<str>,
{
    let body = format!("oid={}&fb_auth={}", oid.as_ref(), fb_auth.as_ref());

    let res = minreq::post(LC_JWT_URL)
        .with_timeout(REQUEST_TIMEOUT_SECS)
        .with_header("Content-Type", "application/x-www-form-urlencoded")
        .with_body(body)
        .send()
        .with_context(|| format!("Failed to connect to authorization server at {LC_JWT_URL}"))?;

    let resp: LcJwtResponse = res
        .json()
        .context("Jwt endpoint returned invalid JSON response")?;

    Ok(resp.jwt)
}

pub fn org_available<T, S>(token: T, name: S) -> Result<bool>
where
    T: AsRef<str>,
    S: AsRef<str>,
{
    let url = format!("{LC_API_URL}/orgs/new?name={}", name.as_ref());
    let bearer = format!("Bearer {}", token.as_ref());

    let res = minreq::get(&url)
        .with_header("Authorization", bearer)
        .send()
        .with_context(|| format!("Failed to query {} availability {url}", name.as_ref()))?;

    let resp: LcOrgAvailable = res
        .json()
        .context("Unable to deserialized data from {url}")?;

    Ok(resp.is_available)
}

pub fn org_create<T, N, L>(token: T, name: N, location: L) -> Result<String>
where
    T: AsRef<str>,
    N: AsRef<str>,
    L: AsRef<str>,
{
    let url = format!("{LC_API_URL}/orgs/new");
    let bearer = format!("Bearer {}", token.as_ref());
    let body = format!("loc={}&name={}&template=", location.as_ref(), name.as_ref());

    let res = minreq::post(&url)
        .with_timeout(REQUEST_TIMEOUT_SECS)
        .with_header("Authorization", bearer)
        .with_header("Content-Type", "application/x-www-form-urlencoded")
        .with_body(body)
        .send()
        .with_context(|| format!("Failed to create org at {url}"))?;

    let resp: OrgCreateResponse = res
        .json()
        .context("Unable to deserialize org creation response")?;

    Ok(resp.data.oid)
}

pub fn get_org_urls<O>(oid: O) -> Result<OrgUrls>
where
    O: AsRef<str>,
{
    let url = format!("{LC_API_URL}/orgs/{}/url", oid.as_ref());

    let res = minreq::get(&url)
        .with_timeout(REQUEST_TIMEOUT_SECS)
        .send()
        .with_context(|| format!("Failed to get org URLs from {url}"))?;

    if res.status_code >= 400 {
        let error_body = res.as_str().unwrap_or("Unknown error");
        anyhow::bail!(
            "Get org URLs failed with status {}: {}",
            res.status_code,
            error_body
        );
    }

    let resp: OrgUrlsResponse = res
        .json()
        .context("Unable to deserialize org URLs response")?;

    Ok(resp.url)
}

pub fn create_installation_key<T, O>(token: T, oid: O, desc: &str) -> Result<String>
where
    T: AsRef<str>,
    O: AsRef<str>,
{
    let url = format!("{LC_API_URL}/installationkeys/{}", oid.as_ref());
    let bearer = format!("Bearer {}", token.as_ref());

    let body = format!("tags={PROJECT_NAME}&desc={desc}");

    let res = minreq::post(&url)
        .with_timeout(REQUEST_TIMEOUT_SECS)
        .with_header("Authorization", bearer)
        .with_header("Content-Type", "application/x-www-form-urlencoded")
        .with_body(body)
        .send()
        .with_context(|| format!("Failed to create installation key at {url}"))?;

    if res.status_code >= 400 {
        let error_body = res.as_str().unwrap_or("Unknown error");
        anyhow::bail!(
            "Installation key creation failed with status {}: {}",
            res.status_code,
            error_body
        );
    }

    let resp: InstallationKeyResponse = res
        .json()
        .context("Unable to deserialize installation key response")?;

    Ok(resp.iid)
}

impl WebhookAdapter<'_> {
    pub fn create(&self) -> Result<()> {
        let url = format!(
            "{LC_API_URL}/hive/cloud_sensor/{}/{}/data",
            self.oid, self.name
        );
        let bearer = format!("Bearer {}", self.token);

        let data = WebhookAdapterData {
            sensor_type: "webhook",
            webhook: WebhookConfig {
                secret: self.secret,
                client_options: ClientOptions {
                    hostname: self.name,
                    identity: Identity {
                        oid: self.oid,
                        installation_key: self.installation_key,
                    },
                    platform: "json",
                    sensor_seed_key: self.sensor_seed_key,
                    mapping: Mapping {
                        event_type_path: "meta_data/type",
                        sensor_key_path: "meta_data/installation_id",
                        sensor_hostname_path: "meta_data/hostname",
                    },
                },
            },
        };

        let data_json =
            serde_json::to_string(&data).context("Failed to serialize webhook adapter data")?;
        let body = format!("data={data_json}");

        let res = minreq::post(&url)
            .with_timeout(REQUEST_TIMEOUT_SECS)
            .with_header("Authorization", bearer)
            .with_header("Content-Type", "application/x-www-form-urlencoded")
            .with_body(body)
            .send()
            .with_context(|| format!("Failed to create webhook adapter at {url}"))?;

        if res.status_code >= 400 {
            let error_body = res.as_str().unwrap_or("Unknown error");
            anyhow::bail!(
                "Webhook adapter creation failed with status {}: {}",
                res.status_code,
                error_body
            );
        }

        Ok(())
    }
}
