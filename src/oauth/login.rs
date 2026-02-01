use std::time::Duration;

use anyhow::{Context, Result};
use colored::Colorize;
use inquire::Text;
use log::info;

use crate::{
    cloud::lc_api::{
        WebhookAdapter, create_installation_key, get_jwt_firebase, get_org_info, get_org_urls,
        org_available, org_create, signup_user,
    },
    common::PROJECT_NAME,
    config::{Config, LcOrg},
    default::get_embedded_default,
    oauth::{LoginArgs, authorize},
};

const ORG_CREATE_TIMEOUT: Duration = Duration::from_secs(120);

const ORG_DEFAULT_LOCATION: &str = "auto";

fn query_user(prompt: &str) -> Result<String> {
    let input = Text::new(prompt)
        .with_validator(|s: &str| {
            if s.trim().is_empty() {
                Ok(inquire::validator::Validation::Invalid(
                    "Input cannot be empty".into(),
                ))
            } else {
                Ok(inquire::validator::Validation::Valid)
            }
        })
        .prompt()
        .context("Failed to read user input")?;

    //
    // add a suffix for the org
    //
    let uuid = uuid::Uuid::new_v4().simple().to_string();
    let suffix = uuid.get(..8).unwrap_or(&uuid);
    let input = format!("{input}-{suffix}-vr");

    Ok(input)
}

fn query_org_name(token: &str) -> Result<String> {
    loop {
        let org_name = query_user("Enter Team Name:")?;

        let available = org_available(token, &org_name)?;

        if available {
            return Ok(org_name);
        }

        println!("{}", format!("{org_name} isn't available").red());
    }
}

fn wait_for_org(oid: &str, token: &str) -> Result<String> {
    use std::time::Instant;

    let start = Instant::now();
    let retry_interval = Duration::from_secs(5);

    loop {
        match get_jwt_firebase(oid, token) {
            Ok(jwt) => return Ok(jwt),
            Err(e) => {
                if start.elapsed() >= ORG_CREATE_TIMEOUT {
                    return Err(e).context("Unable to get JWT TOKEN (timed out after 2 minutes)");
                }
                info!("Waiting for org to be ready... retrying in 5 seconds");
                std::thread::sleep(retry_interval);
            }
        }
    }
}

/// Creates a webhook adapter and returns the full webhook URL.
///
/// The webhook URL format is: <https://{hooks_domain}/{oid}/{adapter_name}/{secret}>
fn create_web_hook(oid: &str, jwt: &str, install_id: &str) -> Result<String> {
    //
    // Query org URLs to get the hooks domain
    //
    info!("Querying org URLs for oid={oid}");
    let urls = get_org_urls(oid).context("Failed to get org URLs")?;
    let hooks_domain = urls
        .hooks
        .context("Hook URL not available for this organization")?;
    info!("Hooks domain: {hooks_domain}");

    //
    // Create an installation key for the webhook adapter
    //
    info!("Creating installation key for webhook adapter");
    let key_desc = format!("{PROJECT_NAME} webhook adapter installation key");
    let installation_key = create_installation_key(jwt, oid, &key_desc)
        .context("Failed to create installation key")?;
    info!("Installation key created: {installation_key}");

    //
    // Generate a secret for the webhook URL
    //
    let secret = uuid::Uuid::new_v4().to_string();

    //
    // Create the webhook adapter in the cloud_sensor hive
    // Using install_id as sensor_seed_key so each installation gets a unique sensor
    //
    info!("Creating webhook adapter for oid={oid}");
    WebhookAdapter::builder()
        .token(jwt)
        .oid(oid)
        .name(PROJECT_NAME)
        .secret(&secret)
        .installation_key(&installation_key)
        .sensor_seed_key(install_id)
        .enabled(true)
        .build()
        .create()
        .context("Failed to create webhook adapter")?;
    info!("Webhook adapter created successfully");

    //
    // Construct the full webhook URL
    //
    let webhook_url = format!("https://{hooks_domain}/{oid}/{PROJECT_NAME}/{secret}");
    info!("Webhook URL: {webhook_url}");

    Ok(webhook_url)
}

////////////////////////////////////////////////////////////////////////////////
// Public
////////////////////////////////////////////////////////////////////////////////

pub fn login(args: &LoginArgs) -> Result<()> {
    println!("Starting authentication...");
    let login = authorize(args)?;
    println!("Authentication successful.");

    //
    // Create LimaCharlie user profile if this is a new user.
    // This calls the same signUp Cloud Function that the web frontend uses.
    // The function is safe to call for existing users - it will return early.
    //
    if let Some(ref email) = login.email {
        println!("Setting up user profile...");
        info!("Creating user profile for {email}");
        signup_user(&login.id_token, email).context("Failed to create user profile")?;
    } else {
        info!("No email in OAuth response, skipping user profile creation");
    }

    println!("Requesting access token...");
    info!("requesting a JWT token");
    let jwt = get_jwt_firebase("-", &login.id_token).context("Unable to get JWT from FB TOKEN")?;
    info!("received token");
    println!("Access token received.");

    //
    // Either use an existing org or create a new one
    //
    let (oid, org_name) = if let Some(ref existing_oid) = args.existing_org {
        // Use existing org - fetch name from API
        println!("Looking up existing organization...");
        info!("Using existing org oid={existing_oid}");
        let org_info =
            get_org_info(&jwt, existing_oid).context("Unable to get organization info")?;
        info!("Org name: {}", org_info.name);
        println!("Using team '{}'.", org_info.name);
        (existing_oid.clone(), org_info.name)
    } else {
        // Ask the user for the org name
        let org_name = query_org_name(&jwt)?;

        // It's an optional parameter
        let location = ORG_DEFAULT_LOCATION;

        // Creating the organization
        println!("Creating team '{org_name}'...");
        info!("Creating {org_name} org in {location}");
        let oid =
            org_create(&jwt, &org_name, location).context("Unable to create organization")?;
        info!("Org created oid={oid}");
        println!("Team created.");
        (oid, org_name)
    };

    //
    // get the final token
    //
    println!("Finalizing setup...");
    info!("requesting a JWT token for {oid}");
    let jwt = wait_for_org(&oid, &login.id_token)?;
    info!("received token");

    let mut config = Config::load()?;
    let url = create_web_hook(&oid, &jwt, &config.install_id)?;

    //
    // save the token to the config file
    //
    println!("Saving configuration...");
    let org = LcOrg {
        oid,
        name: org_name,
        url,
    };
    config.org = org;
    config.save()?;

    let join_curl = get_embedded_default("join_team_command");
    let join_command = format!("{} {}", join_curl, config.org.url);
    let team_url = format!("https://app.limacharlie.io/viberails/teams/{}", config.org.oid);

    println!();
    println!("  {}", "═".repeat(60).as_str().dimmed());
    println!();
    println!("  {} Setup complete!", "✓".green().bold());
    println!();
    println!("  Team: {}", config.org.name.cyan().bold());
    println!("  View: {}", team_url.cyan());
    println!();
    println!("  {}", "─".repeat(60).as_str().dimmed());
    println!();
    println!("  {} Add other machines to this team:", "→".blue());
    println!();
    println!("    {}", join_command.cyan());
    println!();
    println!("  {}", "─".repeat(60).as_str().dimmed());
    println!();
    println!(
        "  Powered by {} {}",
        "LimaCharlie".magenta().bold(),
        "https://limacharlie.io".dimmed()
    );
    println!();
    println!("  {} {}", "Terms of Service:".dimmed(), "https://app.limacharlie.io/tos".dimmed());
    println!();
    println!(
        "  {}",
        "TL;DR: Your data is your own—not sold, accessed, or monetized by".dimmed()
    );
    println!(
        "  {}",
        "LimaCharlie. Your team is created as a Community Edition Organization,".dimmed()
    );
    println!(
        "  {}",
        "completely free. Only limitation is over global throughput. Data is".dimmed()
    );
    println!(
        "  {}",
        "retained for 1 year unless you destroy the Organization.".dimmed()
    );
    println!();
    println!("  {}", "═".repeat(60).as_str().dimmed());
    println!();

    Ok(())
}
