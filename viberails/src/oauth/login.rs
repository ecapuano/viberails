use std::time::Duration;

use anyhow::{Context, Result};
use colored::Colorize;
use inquire::{Select, Text};
use log::info;

use crate::{
    cloud::lc_api::{get_jwt_firebase, org_available, org_create},
    config::{Config, LcOrg},
    oauth::{Location, LoginArgs, authorize},
};

const ORG_CREATE_TIMEOUT: Duration = Duration::from_mins(2);

const LOCATIONS: &[Location] = &[
    Location::Canada,
    Location::India,
    Location::Usa,
    Location::Europe,
    Location::Exp,
    Location::Uk,
    Location::Australia,
];

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

    Ok(input)
}

fn query_location() -> Result<Location> {
    let location = Select::new("Select location:", LOCATIONS.to_vec())
        .prompt()
        .context("Failed to select location")?;

    Ok(location)
}

fn query_org_name(args: &LoginArgs, token: &str) -> Result<String> {
    let mut org_name = if let Some(team_name) = &args.team_name {
        team_name.clone()
    } else {
        query_user("Enter Team Name:")?
    };

    loop {
        let available = org_available(token, &org_name)?;

        if available {
            return Ok(org_name);
        }

        println!("{}", format!("{org_name} isn't available").red());

        org_name = query_user("Enter Team Name:")?;
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

////////////////////////////////////////////////////////////////////////////////
// Public
////////////////////////////////////////////////////////////////////////////////

pub fn login(args: &LoginArgs) -> Result<()> {
    let login = authorize(args)?;

    info!("requesting a JWT token");
    let jwt = get_jwt_firebase("-", &login.id_token).context("Unable to get JWT from FB TOKEN")?;
    info!("received token");

    //
    // Ask the user for the org name
    //
    let org_name = query_org_name(args, &jwt)?;

    //
    // It's an optional parameter
    //
    let location = if let Some(loc) = args.team_location {
        loc
    } else {
        query_location()?
    };

    //
    // Creating the organization
    //
    info!("Creating {org_name} org in {location}");
    let oid = org_create(&jwt, &org_name, location.to_string())
        .context("Unable to create organization")?;
    info!("Org created oid={oid}");

    //
    // get the final token
    //
    info!("requesting a JWT token for {oid}");
    info!("received token");
    let jwt = wait_for_org(&oid, &login.id_token)?;

    //
    // save the token to the config file
    //
    let mut config = Config::load()?;
    let org = LcOrg { oid, jwt };
    config.org = org;
    config.save()
}
