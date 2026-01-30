use anyhow::{Context, Result};
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
