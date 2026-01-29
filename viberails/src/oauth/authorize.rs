//! OAuth authorization flow using Firebase authentication.
//!
//! This implementation uses Firebase's createAuthUri approach which eliminates
//! the need to manage OAuth provider credentials directly. Instead of handling
//! Google OAuth ourselves, we let Firebase manage the OAuth flow.

use anyhow::{Context, Result, anyhow};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use tiny_http::{Response, Server};
use url::Url;

/// Firebase API key (public key - not a secret)
const FIREBASE_API_KEY: &str = "AIzaSyB5VyO6qS-XlnVD3zOIuEVNBD5JFn22_1w";

/// Firebase Auth API endpoints
const CREATE_AUTH_URI: &str = "https://identitytoolkit.googleapis.com/v1/accounts:createAuthUri";
const SIGN_IN_WITH_IDP: &str = "https://identitytoolkit.googleapis.com/v1/accounts:signInWithIdp";
const MFA_FINALIZE: &str = "https://identitytoolkit.googleapis.com/v2/accounts/mfaSignIn:finalize";

/// OAuth callback timeout in seconds
const OAUTH_CALLBACK_TIMEOUT: u64 = 300;

/// Preferred ports for OAuth callback server
const PREFERRED_PORTS: &[u16] = &[8085, 8086, 8087, 8088, 8089];

/// OAuth provider identifiers
#[derive(Debug, Clone, Copy, Default)]
pub enum OAuthProvider {
    #[default]
    Google,
    Microsoft,
}

impl OAuthProvider {
    fn as_firebase_id(&self) -> &'static str {
        match self {
            Self::Google => "google.com",
            Self::Microsoft => "microsoft.com",
        }
    }
}

/// Result of a successful OAuth authorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub id_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
}

/// Request payload for Firebase createAuthUri
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateAuthUriRequest {
    provider_id: String,
    continue_uri: String,
    auth_flow_type: String,
    oauth_scope: String,
}

/// Response from Firebase createAuthUri
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateAuthUriResponse {
    session_id: String,
    auth_uri: String,
}

/// Request payload for Firebase signInWithIdp
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SignInWithIdpRequest {
    request_uri: String,
    post_body: String,
    session_id: String,
    return_secure_token: bool,
    return_idp_credential: bool,
}

/// Response from Firebase signInWithIdp (success case - no MFA)
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignInWithIdpResponse {
    id_token: String,
    refresh_token: String,
    expires_in: String,
    local_id: Option<String>,
}

/// Response from Firebase signInWithIdp when MFA is required
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignInWithIdpMfaResponse {
    mfa_pending_credential: String,
    mfa_info: Vec<MfaFactorInfo>,
    local_id: Option<String>,
}

/// MFA factor information
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct MfaFactorInfo {
    mfa_enrollment_id: String,
    display_name: Option<String>,
    #[serde(default)]
    totp_info: Option<serde_json::Value>,
    #[serde(default)]
    phone_info: Option<String>,
}

/// Request payload for MFA finalize
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MfaFinalizeRequest {
    mfa_pending_credential: String,
    mfa_enrollment_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    totp_verification_info: Option<TotpVerificationInfo>,
}

/// TOTP verification info for MFA
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TotpVerificationInfo {
    verification_code: String,
}

/// Response from MFA finalize
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MfaFinalizeResponse {
    id_token: String,
    refresh_token: String,
    #[serde(default)]
    expires_in: Option<String>,
}

/// Callback result from the OAuth server
enum CallbackResult {
    Success(String), // The full callback path with query params
    Error(String),
    Timeout,
}

/// Configuration for the authorize flow
#[derive(Debug, Clone, Default)]
pub struct AuthorizeConfig {
    pub provider: OAuthProvider,
    pub no_browser: bool,
}

/// Perform OAuth authorization flow.
///
/// This function:
/// 1. Starts a local HTTP server to receive the OAuth callback
/// 2. Requests an auth URI from Firebase
/// 3. Opens the browser (or prints the URL) for user authentication
/// 4. Waits for the callback with the authorization code
/// 5. Exchanges the code for Firebase tokens
///
/// # Arguments
/// * `config` - Configuration for the authorization flow
///
/// # Returns
/// * `Ok(OAuthTokens)` - The OAuth tokens on success
/// * `Err` - An error if authorization fails
pub fn authorize(config: AuthorizeConfig) -> Result<OAuthTokens> {
    // Find a free port and start the callback server
    let port = find_free_port()?;
    let redirect_uri = format!("http://localhost:{port}/callback");

    println!("OAuth callback server started on port {port}");
    println!("Using OAuth provider: {}", config.provider.as_firebase_id());

    // Start the callback server in a separate thread
    let (tx, rx): (Sender<CallbackResult>, Receiver<CallbackResult>) = mpsc::channel();
    let server = Server::http(format!("127.0.0.1:{port}"))
        .map_err(|e| anyhow!("Failed to start OAuth callback server: {e}"))?;

    let server_handle = thread::spawn(move || {
        run_callback_server(server, tx);
    });

    // Get auth URI from Firebase
    let (session_id, auth_uri) = create_auth_uri(&config.provider, &redirect_uri)?;

    // Open browser or print URL
    if config.no_browser {
        println!("\nPlease visit this URL to authenticate:\n{auth_uri}\n");
    } else {
        println!("Opening browser for authentication...");
        if open_browser(&auth_uri).is_err() {
            println!("\nCould not open browser. Please visit this URL:\n{auth_uri}\n");
        }
    }

    println!("Waiting for authentication...");

    // Wait for callback
    let callback_result = rx
        .recv_timeout(Duration::from_secs(OAUTH_CALLBACK_TIMEOUT))
        .unwrap_or(CallbackResult::Timeout);

    // Clean up the server thread (it will exit when the server is dropped)
    drop(server_handle);

    match callback_result {
        CallbackResult::Success(callback_path) => {
            // Extract query string and exchange for Firebase tokens
            let query_string = extract_query_string(&callback_path)?;
            sign_in_with_idp(&redirect_uri, &query_string, &session_id, &config.provider)
        }
        CallbackResult::Error(error) => Err(anyhow!("Authentication failed: {error}")),
        CallbackResult::Timeout => Err(anyhow!("Authentication timeout")),
    }
}

/// Find a free port from the preferred list
fn find_free_port() -> Result<u16> {
    for &port in PREFERRED_PORTS {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(port);
        }
    }

    Err(anyhow!(
        "All OAuth callback ports (8085-8089) are currently in use.\n\
         Please free up one of these ports or close applications using them."
    ))
}

/// Run the OAuth callback server
fn run_callback_server(server: Server, tx: Sender<CallbackResult>) {
    let start_time = Instant::now();
    let timeout = Duration::from_secs(OAUTH_CALLBACK_TIMEOUT);

    for request in server.incoming_requests() {
        // Check timeout
        if start_time.elapsed() > timeout {
            let _ = tx.send(CallbackResult::Timeout);
            return;
        }

        let url = request.url().to_string();

        // Handle favicon requests
        if url == "/favicon.ico" {
            let response = Response::empty(204);
            let _ = request.respond(response);
            continue;
        }

        // Parse the callback URL
        if url.starts_with("/callback") {
            if let Ok(parsed) = Url::parse(&format!("http://localhost{url}")) {
                let params: HashMap<_, _> = parsed.query_pairs().collect();

                if params.contains_key("code") {
                    // Success - send HTML response
                    let html = success_html();
                    let response = Response::from_string(html).with_header(
                        tiny_http::Header::from_bytes(
                            &b"Content-Type"[..],
                            &b"text/html; charset=utf-8"[..],
                        )
                        .expect("valid header"),
                    );
                    let _ = request.respond(response);
                    let _ = tx.send(CallbackResult::Success(url));
                    return;
                } else if let Some(error) = params.get("error") {
                    let error_desc = params
                        .get("error_description")
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| error.to_string());

                    let html = error_html(&error_desc);
                    let response = Response::from_string(html).with_header(
                        tiny_http::Header::from_bytes(
                            &b"Content-Type"[..],
                            &b"text/html; charset=utf-8"[..],
                        )
                        .expect("valid header"),
                    );
                    let _ = request.respond(response);
                    let _ = tx.send(CallbackResult::Error(error_desc));
                    return;
                }
            }

            // Invalid callback - no code or error
            let response = Response::from_string("Invalid OAuth callback").with_status_code(400);
            let _ = request.respond(response);
            let _ = tx.send(CallbackResult::Error("Invalid OAuth callback".to_string()));
            return;
        }

        // Unknown request
        let response = Response::from_string("Not found").with_status_code(404);
        let _ = request.respond(response);
    }
}

/// Request auth URI from Firebase
fn create_auth_uri(provider: &OAuthProvider, redirect_uri: &str) -> Result<(String, String)> {
    let url = format!("{CREATE_AUTH_URI}?key={FIREBASE_API_KEY}");

    info!(
        "Creating auth URI for provider: {}",
        provider.as_firebase_id()
    );
    debug!("redirect_uri: {}", redirect_uri);

    let payload = CreateAuthUriRequest {
        provider_id: provider.as_firebase_id().to_string(),
        continue_uri: redirect_uri.to_string(),
        auth_flow_type: "CODE_FLOW".to_string(),
        oauth_scope: "openid email profile".to_string(),
    };

    debug!(
        "createAuthUri request payload: {:?}",
        serde_json::to_string(&payload).unwrap_or_default()
    );

    let response = minreq::post(&url)
        .with_json(&payload)?
        .with_timeout(10)
        .send()
        .context("Failed to create auth URI")?;

    let response_body = response.as_str().unwrap_or("(non-utf8 response)");
    debug!("createAuthUri response status: {}", response.status_code);
    debug!("createAuthUri response body: {}", response_body);

    if response.status_code != 200 {
        error!(
            "createAuthUri failed: HTTP {} - {}",
            response.status_code, response_body
        );
        return Err(anyhow!(
            "Failed to create auth URI: HTTP {} - {}",
            response.status_code,
            response_body
        ));
    }

    let data: CreateAuthUriResponse =
        serde_json::from_str(response_body).context("Failed to parse createAuthUri response")?;

    info!("Got auth URI, session_id length: {}", data.session_id.len());
    debug!("auth_uri: {}", data.auth_uri);

    Ok((data.session_id, data.auth_uri))
}

/// Extract query string from callback URL
fn extract_query_string(callback_path: &str) -> Result<String> {
    info!("Extracting query string from callback path");
    debug!("callback_path: {}", callback_path);

    let parsed = Url::parse(&format!("http://localhost{callback_path}"))
        .context("Failed to parse callback URL")?;

    let query = parsed
        .query()
        .ok_or_else(|| anyhow!("No query parameters in callback"))?;

    debug!("Extracted query string: {}", query);

    // Check for error in query
    let params: HashMap<_, _> = parsed.query_pairs().collect();
    debug!("Query parameters: {:?}", params.keys().collect::<Vec<_>>());

    if let Some(error) = params.get("error") {
        let error_desc = params
            .get("error_description")
            .map(|s| s.to_string())
            .unwrap_or_else(|| error.to_string());
        error!("OAuth error in callback: {}", error_desc);
        return Err(anyhow!("OAuth error: {error_desc}"));
    }

    if params.contains_key("code") {
        info!("Found authorization code in callback");
    } else {
        warn!("No 'code' parameter found in callback query string");
    }

    Ok(query.to_string())
}

/// Exchange provider response with Firebase for tokens
fn sign_in_with_idp(
    request_uri: &str,
    query_string: &str,
    session_id: &str,
    provider: &OAuthProvider,
) -> Result<OAuthTokens> {
    let url = format!("{SIGN_IN_WITH_IDP}?key={FIREBASE_API_KEY}");

    info!("Exchanging OAuth callback with Firebase signInWithIdp");
    debug!("request_uri: {}", request_uri);
    debug!("query_string: {}", query_string);
    debug!("session_id: {}", session_id);

    // Pass the full query string from the OAuth callback as post_body
    // Firebase will extract the authorization code and exchange it for tokens
    let payload = SignInWithIdpRequest {
        request_uri: request_uri.to_string(),
        post_body: query_string.to_string(),
        session_id: session_id.to_string(),
        return_secure_token: true,
        return_idp_credential: true,
    };

    debug!(
        "signInWithIdp request payload: {:?}",
        serde_json::to_string(&payload).unwrap_or_default()
    );

    let response = minreq::post(&url)
        .with_json(&payload)?
        .with_timeout(10)
        .send()
        .context("Failed to sign in with IdP")?;

    let response_body = response.as_str().unwrap_or("(non-utf8 response)");
    debug!("signInWithIdp response status: {}", response.status_code);
    debug!("signInWithIdp response body: {}", response_body);

    if response.status_code != 200 {
        error!(
            "signInWithIdp failed: HTTP {} - {}",
            response.status_code, response_body
        );
        return Err(anyhow!(
            "Failed to sign in with IdP: HTTP {} - {}",
            response.status_code,
            response_body
        ));
    }

    // First, check if MFA is required by looking for mfaPendingCredential
    if let Ok(mfa_response) = serde_json::from_str::<SignInWithIdpMfaResponse>(response_body) {
        info!("MFA required, initiating MFA verification flow");
        return handle_mfa_verification(&mfa_response, provider);
    }

    // No MFA required, parse as regular response
    let data: SignInWithIdpResponse =
        serde_json::from_str(response_body).context("Failed to parse signInWithIdp response")?;

    // Calculate expiry timestamp
    let expires_in: i64 = data.expires_in.parse().unwrap_or(3600);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let expires_at = now.saturating_add(expires_in);

    Ok(OAuthTokens {
        id_token: data.id_token,
        refresh_token: data.refresh_token,
        expires_at,
        provider: provider.as_firebase_id().to_string(),
        uid: data.local_id,
    })
}

/// Handle MFA verification flow
fn handle_mfa_verification(
    mfa_response: &SignInWithIdpMfaResponse,
    provider: &OAuthProvider,
) -> Result<OAuthTokens> {
    println!();
    println!("{}", "=".repeat(60));
    println!("Multi-Factor Authentication Required");
    println!("{}", "=".repeat(60));
    println!();
    println!("Your account has 2FA enabled. Please complete verification.");
    println!();
    println!("Enrolled authentication factor(s):");

    for (i, factor) in mfa_response.mfa_info.iter().enumerate() {
        let display_name = factor.display_name.as_deref().unwrap_or("Unnamed factor");
        let factor_type = if factor.phone_info.is_some() {
            "SMS"
        } else if factor.totp_info.is_some() {
            "Authenticator app (TOTP)"
        } else {
            "Unknown"
        };
        println!("  {}. {} ({})", i + 1, display_name, factor_type);
    }
    println!();

    // Use the first factor (typically TOTP)
    let factor = mfa_response
        .mfa_info
        .first()
        .ok_or_else(|| anyhow!("No MFA factors found"))?;

    // Only TOTP is supported for now
    if factor.totp_info.is_none() {
        return Err(anyhow!(
            "Only TOTP (authenticator app) MFA is supported. SMS MFA is not yet implemented."
        ));
    }

    // Prompt for verification code
    let verification_code = prompt_mfa_code(factor)?;

    // Finalize MFA
    finalize_mfa(
        &mfa_response.mfa_pending_credential,
        &factor.mfa_enrollment_id,
        &verification_code,
        provider,
        mfa_response.local_id.as_deref(),
    )
}

/// Prompt user for MFA verification code
fn prompt_mfa_code(factor: &MfaFactorInfo) -> Result<String> {
    let factor_name = factor
        .display_name
        .as_deref()
        .unwrap_or("your authenticator");

    let prompt = format!("Enter the 6-digit code from {}: ", factor_name);

    for attempt in 0..3 {
        if attempt > 0 {
            println!("Please try again.");
        }

        print!("{}", prompt);
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut code = String::new();
        std::io::stdin()
            .read_line(&mut code)
            .context("Failed to read verification code")?;

        let code = code.trim();

        if code.is_empty() {
            println!("Error: Code cannot be empty.");
            continue;
        }

        if !code.chars().all(|c| c.is_ascii_digit()) {
            println!("Error: Code must contain only digits.");
            continue;
        }

        if code.len() != 6 {
            println!(
                "Error: Code must be exactly 6 digits (you entered {}).",
                code.len()
            );
            continue;
        }

        return Ok(code.to_string());
    }

    Err(anyhow!("Maximum attempts (3) exceeded"))
}

/// Finalize MFA verification and get tokens
fn finalize_mfa(
    mfa_pending_credential: &str,
    mfa_enrollment_id: &str,
    verification_code: &str,
    provider: &OAuthProvider,
    local_id: Option<&str>,
) -> Result<OAuthTokens> {
    let url = format!("{MFA_FINALIZE}?key={FIREBASE_API_KEY}");

    info!("Finalizing MFA verification");

    let payload = MfaFinalizeRequest {
        mfa_pending_credential: mfa_pending_credential.to_string(),
        mfa_enrollment_id: mfa_enrollment_id.to_string(),
        totp_verification_info: Some(TotpVerificationInfo {
            verification_code: verification_code.to_string(),
        }),
    };

    println!("Verifying code...");

    let response = minreq::post(&url)
        .with_json(&payload)?
        .with_timeout(15)
        .send()
        .context("Failed to finalize MFA")?;

    let response_body = response.as_str().unwrap_or("(non-utf8 response)");
    info!("MFA finalize response status: {}", response.status_code);
    info!("MFA finalize response body: {}", response_body);

    if response.status_code != 200 {
        // Parse error message
        let error_msg =
            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(response_body) {
                error_json["error"]["message"]
                    .as_str()
                    .unwrap_or("Unknown error")
                    .to_string()
            } else {
                response_body.to_string()
            };

        if error_msg.contains("INVALID_CODE") || error_msg.contains("CODE_EXPIRED") {
            return Err(anyhow!("Invalid or expired verification code."));
        } else if error_msg.contains("INVALID_MFA_PENDING_CREDENTIAL") {
            return Err(anyhow!("MFA session expired. Please try logging in again."));
        } else if error_msg.contains("TOO_MANY_ATTEMPTS") {
            return Err(anyhow!(
                "Too many failed attempts. Please try logging in again."
            ));
        }

        return Err(anyhow!("MFA verification failed: {}", error_msg));
    }

    let data: MfaFinalizeResponse =
        serde_json::from_str(response_body).context("Failed to parse MFA finalize response")?;

    println!("Verification successful!");

    // Calculate expiry timestamp (default to 1 hour if not provided)
    let expires_in: i64 = data
        .expires_in
        .as_ref()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3600);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let expires_at = now.saturating_add(expires_in);

    Ok(OAuthTokens {
        id_token: data.id_token,
        refresh_token: data.refresh_token,
        expires_at,
        provider: provider.as_firebase_id().to_string(),
        uid: local_id.map(String::from),
    })
}

/// Open URL in the default browser
fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .context("Failed to open browser")?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .context("Failed to open browser")?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .context("Failed to open browser")?;
    }

    Ok(())
}

/// Generate success HTML page
fn success_html() -> String {
    r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>LimaCharlie - Authentication Successful</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #00030C;
            color: #ffffff;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            background-image:
                radial-gradient(circle at 20% 50%, rgba(74, 144, 226, 0.1) 0%, transparent 50%),
                radial-gradient(circle at 80% 80%, rgba(226, 74, 144, 0.08) 0%, transparent 50%);
        }
        .container {
            text-align: center;
            padding: 60px 40px;
            background: rgba(255, 255, 255, 0.05);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 16px;
            backdrop-filter: blur(24px);
            box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
            max-width: 500px;
            width: 90%;
        }
        .success-icon {
            width: 80px;
            height: 80px;
            margin: 0 auto 30px;
            background: linear-gradient(135deg, #4A90E2 0%, #A74AE2 100%);
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 40px;
            box-shadow: 0 4px 20px rgba(74, 144, 226, 0.4);
        }
        .title {
            font-size: 28px;
            font-weight: 500;
            margin-bottom: 16px;
        }
        .message {
            font-size: 16px;
            color: rgba(255, 255, 255, 0.7);
            line-height: 1.6;
            margin-bottom: 40px;
        }
        .cli-hint {
            margin-top: 40px;
            padding: 16px;
            background: rgba(255, 255, 255, 0.05);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 8px;
            font-family: 'Courier New', monospace;
            font-size: 14px;
            color: #4A90E2;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="success-icon">&#10003;</div>
        <h1 class="title">Authentication Successful</h1>
        <p class="message">
            You've been successfully authenticated.<br>
            Your credentials have been securely stored.
        </p>
        <p class="message">You can safely close this browser window/tab.</p>
        <div class="cli-hint">Return to your terminal to continue</div>
    </div>
</body>
</html>"#
        .to_string()
}

/// Generate error HTML page
fn error_html(error_msg: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>LimaCharlie - Authentication Error</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #00030C;
            color: #ffffff;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            background-image:
                radial-gradient(circle at 20% 50%, rgba(226, 74, 74, 0.1) 0%, transparent 50%),
                radial-gradient(circle at 80% 80%, rgba(226, 74, 74, 0.08) 0%, transparent 50%);
        }}
        .container {{
            text-align: center;
            padding: 60px 40px;
            background: rgba(255, 255, 255, 0.05);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 16px;
            backdrop-filter: blur(24px);
            box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
            max-width: 500px;
            width: 90%;
        }}
        .error-icon {{
            width: 80px;
            height: 80px;
            margin: 0 auto 30px;
            background: linear-gradient(135deg, #E24A4A 0%, #F02463 100%);
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 40px;
            box-shadow: 0 4px 20px rgba(226, 74, 74, 0.4);
        }}
        .title {{
            font-size: 28px;
            font-weight: 500;
            margin-bottom: 16px;
        }}
        .error-message {{
            font-size: 16px;
            color: #E24A4A;
            margin-bottom: 16px;
            padding: 12px 20px;
            background: rgba(226, 74, 74, 0.1);
            border: 1px solid rgba(226, 74, 74, 0.2);
            border-radius: 8px;
            font-family: 'Courier New', monospace;
        }}
        .message {{
            font-size: 16px;
            color: rgba(255, 255, 255, 0.7);
            line-height: 1.6;
            margin-bottom: 40px;
        }}
        .cli-hint {{
            margin-top: 40px;
            padding: 16px;
            background: rgba(255, 255, 255, 0.05);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 8px;
            font-family: 'Courier New', monospace;
            font-size: 14px;
            color: #4A90E2;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="error-icon">&#10007;</div>
        <h1 class="title">Authentication Failed</h1>
        <div class="error-message">{error_msg}</div>
        <p class="message">
            The authentication process encountered an error.<br>
            Please return to your terminal and try again.
        </p>
        <p class="message">You can close this browser window/tab.</p>
        <div class="cli-hint">Return to your terminal to retry</div>
    </div>
</body>
</html>"#
    )
}
