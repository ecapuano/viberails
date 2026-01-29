//! OAuth authorization flow using Firebase authentication.
//!
//! This implementation uses Firebase's createAuthUri approach which eliminates
//! the need to manage OAuth provider credentials directly. Instead of handling
//! Google OAuth ourselves, we let Firebase manage the OAuth flow.

use anyhow::{Context, Result, anyhow};
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

/// Response from Firebase signInWithIdp
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignInWithIdpResponse {
    id_token: String,
    refresh_token: String,
    expires_in: String,
    local_id: Option<String>,
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
            // Extract query string and exchange for tokens
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
            // Check for authorization code or error
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

            // Invalid callback
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

    let payload = CreateAuthUriRequest {
        provider_id: provider.as_firebase_id().to_string(),
        continue_uri: redirect_uri.to_string(),
        auth_flow_type: "CODE_FLOW".to_string(),
        oauth_scope: "openid email profile".to_string(),
    };

    let response = minreq::post(&url)
        .with_json(&payload)?
        .with_timeout(10)
        .send()
        .context("Failed to create auth URI")?;

    if response.status_code != 200 {
        return Err(anyhow!(
            "Failed to create auth URI: HTTP {}",
            response.status_code
        ));
    }

    let data: CreateAuthUriResponse = response
        .json()
        .context("Failed to parse createAuthUri response")?;

    Ok((data.session_id, data.auth_uri))
}

/// Extract query string from callback URL
fn extract_query_string(callback_path: &str) -> Result<String> {
    let parsed = Url::parse(&format!("http://localhost{callback_path}"))
        .context("Failed to parse callback URL")?;

    let query = parsed
        .query()
        .ok_or_else(|| anyhow!("No query parameters in callback"))?;

    // Check for error in query
    let params: HashMap<_, _> = parsed.query_pairs().collect();
    if let Some(error) = params.get("error") {
        let error_desc = params
            .get("error_description")
            .map(|s| s.to_string())
            .unwrap_or_else(|| error.to_string());
        return Err(anyhow!("OAuth error: {error_desc}"));
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

    let payload = SignInWithIdpRequest {
        request_uri: request_uri.to_string(),
        post_body: query_string.to_string(),
        session_id: session_id.to_string(),
        return_secure_token: true,
        return_idp_credential: true,
    };

    let response = minreq::post(&url)
        .with_json(&payload)?
        .with_timeout(10)
        .send()
        .context("Failed to sign in with IdP")?;

    if response.status_code != 200 {
        return Err(anyhow!(
            "Failed to sign in with IdP: HTTP {}",
            response.status_code
        ));
    }

    let data: SignInWithIdpResponse = response
        .json()
        .context("Failed to parse signInWithIdp response")?;

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
