use anyhow::{Context, Result};
use bon::Builder;
use serde::Deserialize;

use crate::cloud::REQUEST_TIMEOUT_SECS;

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

#[derive(Builder)]
pub struct OutputCreate<'a> {
    token: &'a str,
    oid: &'a str,
    name: &'a str,
    module: &'a str,
    output_type: &'a str,
    dest_host: &'a str,
}

impl OutputCreate<'_> {
    pub fn create(&self) -> Result<()> {
        let url = format!("{LC_API_URL}/outputs/{}", self.oid);
        let bearer = format!("Bearer {}", self.token);

        let body = format!(
            "name={}&module={}&type={}&dest_host={}",
            self.name, self.module, self.output_type, self.dest_host
        );

        let res = minreq::post(&url)
            .with_timeout(REQUEST_TIMEOUT_SECS)
            .with_header("Authorization", bearer)
            .with_header("Content-Type", "application/x-www-form-urlencoded")
            .with_body(body)
            .send()
            .with_context(|| format!("Failed to create output at {url}"))?;

        if res.status_code >= 400 {
            let error_body = res.as_str().unwrap_or("Unknown error");
            anyhow::bail!(
                "Output creation failed with status {}: {}",
                res.status_code,
                error_body
            );
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct OrgUrlsResponse {
    url: OrgUrls,
}

#[derive(Debug, Deserialize)]
pub struct OrgUrls {
    pub hooks: Option<String>,
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
