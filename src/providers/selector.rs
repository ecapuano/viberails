use std::fmt;

use anyhow::{Result, bail};
use colored::Colorize;
use inquire::{MultiSelect, validator::Validation};

use super::discovery::DiscoveryResult;
use super::registry::ProviderRegistry;

/// Display mode for the selectable provider
#[derive(Clone, Copy)]
enum SelectionMode {
    /// For install: select based on whether tool is detected
    Install,
    /// For uninstall: select based on whether our hooks are installed
    Uninstall,
}

/// A selectable item in the multi-select UI
struct SelectableProvider {
    result: DiscoveryResult,
    mode: SelectionMode,
}

impl SelectableProvider {
    fn new(result: DiscoveryResult, mode: SelectionMode) -> Self {
        Self { result, mode }
    }

    fn is_selectable(&self) -> bool {
        match self.mode {
            SelectionMode::Install => self.result.detected,
            SelectionMode::Uninstall => self.result.hooks_installed,
        }
    }
}

impl fmt::Display for SelectableProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.mode {
            SelectionMode::Install => {
                if self.result.detected {
                    write!(f, "{} {}", self.result.display_name, "[detected]".green())
                } else {
                    write!(
                        f,
                        "{} {}",
                        self.result.display_name.dimmed(),
                        "[not found]".dimmed()
                    )
                }
            }
            SelectionMode::Uninstall => {
                if self.result.hooks_installed {
                    write!(f, "{} {}", self.result.display_name, "[installed]".green())
                } else if self.result.detected {
                    write!(
                        f,
                        "{} {}",
                        self.result.display_name.dimmed(),
                        "[available but not installed]".dimmed()
                    )
                } else {
                    write!(
                        f,
                        "{} {}",
                        self.result.display_name.dimmed(),
                        "[not found]".dimmed()
                    )
                }
            }
        }
    }
}

/// Result of the selection process
pub struct SelectionResult {
    /// IDs of the selected providers
    pub selected_ids: Vec<&'static str>,
}

/// Internal helper to run the multi-select UI for install
fn run_install_selection(
    registry: &ProviderRegistry,
    prompt_text: &str,
    no_tools_message: &str,
    no_tools_error: &str,
) -> Result<Option<SelectionResult>> {
    let discoveries = registry.discover_all();

    if discoveries.is_empty() {
        bail!("No providers registered in the registry");
    }

    // Check if any providers are detected
    let any_detected = discoveries.iter().any(|d| d.detected);
    if !any_detected {
        println!("\n{}", no_tools_message.yellow());
        println!("\nSupported tools and installation hints:");
        for d in &discoveries {
            println!(
                "  {} - {}",
                d.display_name,
                d.detection_hint
                    .as_deref()
                    .unwrap_or("No installation hint available")
            );
        }
        bail!("{no_tools_error}");
    }

    let options: Vec<SelectableProvider> = discoveries
        .into_iter()
        .map(|d| SelectableProvider::new(d, SelectionMode::Install))
        .collect();

    // Pre-select all detected providers
    let default_indices: Vec<usize> = options
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_selectable())
        .map(|(i, _)| i)
        .collect();

    // Create a validator that ensures only detected providers are selected
    // and at least one is selected
    let validator = |selections: &[inquire::list_option::ListOption<&SelectableProvider>]| {
        if selections.is_empty() {
            return Ok(Validation::Invalid(
                "Please select at least one tool".into(),
            ));
        }

        for selection in selections {
            if !selection.value.is_selectable() {
                return Ok(Validation::Invalid(
                    format!(
                        "{} is not installed and cannot be selected",
                        selection.value.result.display_name
                    )
                    .into(),
                ));
            }
        }

        Ok(Validation::Valid)
    };

    let prompt = MultiSelect::new(prompt_text, options)
        .with_default(&default_indices)
        .with_validator(validator)
        .with_help_message("Use ↑↓ to navigate, Space to toggle, Enter to confirm");

    match prompt.prompt() {
        Ok(selections) => {
            let selected_ids: Vec<&'static str> =
                selections.into_iter().map(|p| p.result.id).collect();
            Ok(Some(SelectionResult { selected_ids }))
        }
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Internal helper to run the multi-select UI for uninstall
fn run_uninstall_selection(
    registry: &ProviderRegistry,
    prompt_text: &str,
) -> Result<Option<SelectionResult>> {
    let discoveries = registry.discover_all_with_hooks_check();

    if discoveries.is_empty() {
        bail!("No providers registered in the registry");
    }

    // Check if any providers have our hooks installed
    let any_installed = discoveries.iter().any(|d| d.hooks_installed);
    if !any_installed {
        // Show info about detected tools vs installed hooks
        let detected_tools: Vec<_> = discoveries.iter().filter(|d| d.detected).collect();

        if detected_tools.is_empty() {
            println!(
                "\n{}",
                "No supported AI coding tools detected on this system.".yellow()
            );
        } else {
            println!(
                "\n{}",
                "No viberails hooks are installed in any detected tools.".yellow()
            );
            println!("\nDetected tools (hooks not installed):");
            for d in &detected_tools {
                println!("  {} - available but hooks not installed", d.display_name);
            }
        }
        bail!("Nothing to uninstall.");
    }

    let options: Vec<SelectableProvider> = discoveries
        .into_iter()
        .map(|d| SelectableProvider::new(d, SelectionMode::Uninstall))
        .collect();

    // Pre-select all providers with hooks installed
    let default_indices: Vec<usize> = options
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_selectable())
        .map(|(i, _)| i)
        .collect();

    // Create a validator that ensures only providers with hooks installed are selected
    // and at least one is selected
    let validator = |selections: &[inquire::list_option::ListOption<&SelectableProvider>]| {
        if selections.is_empty() {
            return Ok(Validation::Invalid(
                "Please select at least one tool".into(),
            ));
        }

        for selection in selections {
            if !selection.value.is_selectable() {
                return Ok(Validation::Invalid(
                    format!(
                        "{} does not have hooks installed and cannot be uninstalled",
                        selection.value.result.display_name
                    )
                    .into(),
                ));
            }
        }

        Ok(Validation::Valid)
    };

    let prompt = MultiSelect::new(prompt_text, options)
        .with_default(&default_indices)
        .with_validator(validator)
        .with_help_message("Use ↑↓ to navigate, Space to toggle, Enter to confirm");

    match prompt.prompt() {
        Ok(selections) => {
            let selected_ids: Vec<&'static str> =
                selections.into_iter().map(|p| p.result.id).collect();
            Ok(Some(SelectionResult { selected_ids }))
        }
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Show a multi-select UI for choosing which providers to install hooks for.
/// Only detected providers can be selected.
/// Returns None if the user cancels.
pub fn select_providers(registry: &ProviderRegistry) -> Result<Option<SelectionResult>> {
    run_install_selection(
        registry,
        "Select AI coding tools to install hooks for:",
        "No supported AI coding tools detected on this system.",
        "No tools detected. Please install a supported AI coding tool first.",
    )
}

/// Show a multi-select UI for choosing which providers to uninstall hooks from.
/// Only providers with hooks installed can be selected.
/// Providers that are detected but don't have hooks are shown as "available but not installed".
/// Returns None if the user cancels.
pub fn select_providers_for_uninstall(
    registry: &ProviderRegistry,
) -> Result<Option<SelectionResult>> {
    run_uninstall_selection(registry, "Select AI coding tools to uninstall hooks from:")
}
