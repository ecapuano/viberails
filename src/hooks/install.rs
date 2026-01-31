use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Error, Result, anyhow, bail};
use colored::Colorize;
use log::{error, info, warn};

use crate::{
    common::{PROJECT_NAME, display_authorize_help, print_header},
    config::{Config, uninstall_config},
    providers::{ProviderRegistry, select_providers, select_providers_for_uninstall},
};

const LABEL_WIDTH: usize = 20;

struct InstallResult {
    provider_name: String,
    hooktype: &'static str,
    result: Result<(), Error>,
}

impl fmt::Display for InstallResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.result {
            Ok(()) => write!(
                f,
                "{:<LABEL_WIDTH$} {:<20} {}",
                self.provider_name,
                self.hooktype,
                "[SUCCESS]".green()
            ),
            Err(e) => write!(
                f,
                "{:<LABEL_WIDTH$} {:<20} {} {}",
                self.provider_name,
                self.hooktype,
                "[FAILURE]".red(),
                e
            ),
        }
    }
}

fn install_hooks_for_provider(
    registry: &ProviderRegistry,
    provider_id: &str,
) -> Vec<InstallResult> {
    let mut results = vec![];

    let Some(factory) = registry.get(provider_id) else {
        results.push(InstallResult {
            provider_name: provider_id.to_string(),
            hooktype: "*",
            result: Err(anyhow!("Unknown provider")),
        });
        return results;
    };

    let provider = match factory.create() {
        Ok(p) => p,
        Err(e) => {
            results.push(InstallResult {
                provider_name: factory.display_name().to_string(),
                hooktype: "*",
                result: Err(e),
            });
            return results;
        }
    };

    for hook_type in factory.supported_hooks() {
        let ret = provider.install(hook_type);

        results.push(InstallResult {
            provider_name: factory.display_name().to_string(),
            hooktype: hook_type,
            result: ret,
        });
    }

    results
}

fn uninstall_hooks_for_provider(
    registry: &ProviderRegistry,
    provider_id: &str,
) -> Vec<InstallResult> {
    let mut results = vec![];

    let Some(factory) = registry.get(provider_id) else {
        return results;
    };

    let provider = match factory.create() {
        Ok(p) => p,
        Err(e) => {
            warn!("Failed to create provider {provider_id}: {e}");
            return results;
        }
    };

    for hook_type in factory.supported_hooks() {
        let ret = provider.uninstall(hook_type);

        results.push(InstallResult {
            provider_name: factory.display_name().to_string(),
            hooktype: hook_type,
            result: ret,
        });
    }

    results
}

fn display_results(results: &[InstallResult]) {
    print_header();
    for r in results {
        println!("{r}");
    }
}

fn install_binary(dst: &Path) -> Result<()> {
    info!("Install location {}", dst.display());

    //
    // We'll try to overwrite regardlless if it exists or not
    //
    let current_exe = env::current_exe().context("Unable to find current exe")?;

    fs::copy(&current_exe, dst).with_context(|| {
        format!(
            "Unable to copy {} to {}",
            current_exe.display(),
            dst.display()
        )
    })?;

    info!("copied to {}", dst.display());

    Ok(())
}

fn uninstall_binary(dst: &Path) -> Result<()> {
    info!("Uninstall location {}", dst.display());

    if !dst.exists() {
        warn!("{} doesn't exist", dst.display());
        return Ok(());
    }

    fs::remove_file(dst).with_context(|| format!("Unable to delete {}", dst.display()))?;

    info!("{} was deleted", dst.display());

    Ok(())
}

fn binary_location() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| {
        anyhow!("Unable to determine home directory. Ensure HOME environment variable is set")
    })?;

    let local_bin = home.join(".local").join("bin");

    if !local_bin.exists() {
        fs::create_dir_all(&local_bin)
            .with_context(|| format!("Unable to create {}", local_bin.display()))?;
    }

    let file_name = if cfg!(target_os = "windows") {
        format!("{PROJECT_NAME}.exe")
    } else {
        PROJECT_NAME.to_string()
    };

    Ok(local_bin.join(file_name))
}

////////////////////////////////////////////////////////////////////////////////
// PUBLIC
////////////////////////////////////////////////////////////////////////////////

pub fn install() -> Result<()> {
    //
    // Make sure we're autorized, otherwise it'll fail silently
    //
    let config = Config::load()?;
    if !config.org.authorized() {
        display_authorize_help();
        bail!("Not Authorized");
    }

    //
    // Create the registry and let the user select providers
    //
    let registry = ProviderRegistry::new();
    let selection = select_providers(&registry)?;

    let Some(selection) = selection else {
        println!("Installation cancelled.");
        return Ok(());
    };

    if selection.selected_ids.is_empty() {
        println!("No providers selected.");
        return Ok(());
    }

    //
    // We also have to install ourselves on the host. We'll do like claude-code
    // and install ourselves in ~/.local/bin/
    //
    // It doesn't matter if it's not in the path since the hook contains
    // an absolute path
    //
    let dst = binary_location()?;
    install_binary(&dst)?;

    //
    // Install hooks for each selected provider
    //
    let mut all_results = Vec::new();

    for provider_id in &selection.selected_ids {
        let results = install_hooks_for_provider(&registry, provider_id);
        all_results.extend(results);
    }

    display_results(&all_results);

    Ok(())
}

pub fn uninstall() -> Result<()> {
    let mut success = true;
    let dst = binary_location()?;

    //
    // Create the registry and let the user select providers to uninstall from
    //
    let registry = ProviderRegistry::new();
    let selection = select_providers_for_uninstall(&registry)?;

    let Some(selection) = selection else {
        println!("Uninstallation cancelled.");
        return Ok(());
    };

    if selection.selected_ids.is_empty() {
        println!("No providers selected.");
        return Ok(());
    }

    //
    // Uninstall hooks for each selected provider
    //
    let mut all_results = Vec::new();

    for provider_id in &selection.selected_ids {
        let results = uninstall_hooks_for_provider(&registry, provider_id);
        all_results.extend(results);
    }

    display_results(&all_results);

    //
    // Only delete binary and config if ALL providers with hooks installed were uninstalled
    //
    let installed_count = registry
        .discover_all_with_hooks_check()
        .iter()
        .filter(|d| d.hooks_installed)
        .count();
    let all_uninstalled = selection.selected_ids.len() >= installed_count;

    if all_uninstalled {
        if let Err(e) = uninstall_binary(&dst) {
            error!("Unable to delete binary ({e}");
            success = false;
        }

        if let Err(e) = uninstall_config() {
            error!("Unable to delete config files ({e}");
            success = false;
        }
    } else {
        println!(
            "\nHooks removed from selected tools. Binary and config retained for remaining tools."
        );
    }

    if success {
        Ok(())
    } else {
        Err(anyhow!("Uninstall failures. See logs"))
    }
}
