use std::time::Duration;

use anyhow::{Context, Result};
use colored::Colorize;
use inquire::Text;
use log::info;

use crate::{
    cloud::lc_api::{OutputCreate, get_jwt_firebase, get_org_urls, org_available, org_create},
    config::{Config, LcOrg},
    oauth::{LoginArgs, authorize},
};

const ORG_CREATE_TIMEOUT: Duration = Duration::from_mins(2);
const ORG_DEFAULT_LOCATION: &str = "usa";

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

fn create_web_hook(oid: &str, jwt: &str, dest_url: &str) -> Result<String> {
    //
    // Create webhook output for detections
    //
    info!("Creating webhook output for oid={oid}");
    OutputCreate::builder()
        .token(jwt)
        .oid(oid)
        .name("detections-vr")
        .module("webhook")
        .output_type("detect")
        .dest_host(dest_url)
        .build()
        .create()
        .context("Failed to create webhook output")?;

    info!("Webhook output created successfully");

    //
    // Query org URLs to get the hook URL
    //
    info!("Querying org URLs for oid={oid}");
    let urls = get_org_urls(oid).context("Failed to get org URLs")?;
    info!("Org URLs: {:?}", urls);

    let hook = urls
        .hooks
        .context("Hook URL not available for this organization")?;

    Ok(hook)
}

////////////////////////////////////////////////////////////////////////////////
// Public
////////////////////////////////////////////////////////////////////////////////

pub fn login(args: &LoginArgs) -> Result<()> {
    println!("Starting authentication...");
    let login = authorize(args)?;
    println!("Authentication successful.");

    println!("Requesting access token...");
    info!("requesting a JWT token");
    let jwt = get_jwt_firebase("-", &login.id_token).context("Unable to get JWT from FB TOKEN")?;
    info!("received token");
    println!("Access token received.");

    //
    // Ask the user for the org name
    //
    let org_name = query_org_name(&jwt)?;

    //
    // It's an optional parameter
    //
    let location = ORG_DEFAULT_LOCATION;

    //
    // Creating the organization
    //
    println!("Creating team '{org_name}'...");
    info!("Creating {org_name} org in {location}");
    let oid = org_create(&jwt, &org_name, location).context("Unable to create organization")?;
    info!("Org created oid={oid}");
    println!("Team created.");

    //
    // get the final token
    //
    println!("Finalizing setup...");
    info!("requesting a JWT token for {oid}");
    let jwt = wait_for_org(&oid, &login.id_token)?;
    info!("received token");

    let url = create_web_hook(&oid, &jwt, "hello")?;

    //
    // save the token to the config file
    //
    println!("Saving configuration...");
    let mut config = Config::load()?;
    let org = LcOrg {
        oid,
        jwt,
        name: org_name,
        url,
    };
    config.org = org;
    config.save()?;

    println!();
    println!("{}", "Login complete! You are now authenticated.".green());

    Ok(())
}
