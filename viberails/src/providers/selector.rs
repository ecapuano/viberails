use std::fmt;

use anyhow::{Result, bail};
use colored::Colorize;
use inquire::{MultiSelect, validator::Validation};

use super::discovery::DiscoveryResult;
use super::registry::ProviderRegistry;

/// A selectable item in the multi-select UI
struct SelectableProvider {
    result: DiscoveryResult,
}

impl SelectableProvider {
    fn new(result: DiscoveryResult) -> Self {
        Self { result }
    }

    fn is_selectable(&self) -> bool {
        self.result.detected
    }
}

impl fmt::Display for SelectableProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.result.detected {
            write!(
                f,
                "{} {}",
                self.result.display_name,
                "[detected]".green()
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

/// Result of the selection process
pub struct SelectionResult {
    /// IDs of the selected providers
    pub selected_ids: Vec<&'static str>,
}

/// Internal helper to run the multi-select UI
fn run_selection(
    registry: &ProviderRegistry,
    prompt_text: &str,
    no_tools_message: &str,
    no_tools_error: &str,
    show_install_hints: bool,
) -> Result<Option<SelectionResult>> {
    let discoveries = registry.discover_all();

    if discoveries.is_empty() {
        bail!("No providers registered in the registry");
    }

    // Check if any providers are detected
    let any_detected = discoveries.iter().any(|d| d.detected);
    if !any_detected {
        println!("\n{}", no_tools_message.yellow());
        if show_install_hints {
            println!("\nSupported tools and installation hints:");
            for d in &discoveries {
                println!("  {} - {}", d.display_name, d.detection_hint.as_deref().unwrap_or("No installation hint available"));
            }
        }
        bail!("{}", no_tools_error);
    }

    let options: Vec<SelectableProvider> = discoveries
        .into_iter()
        .map(SelectableProvider::new)
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
            return Ok(Validation::Invalid("Please select at least one tool".into()));
        }

        for selection in selections {
            if !selection.value.is_selectable() {
                return Ok(Validation::Invalid(
                    format!("{} is not installed and cannot be selected", selection.value.result.display_name).into()
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
            let selected_ids: Vec<&'static str> = selections
                .into_iter()
                .map(|p| p.result.id)
                .collect();
            Ok(Some(SelectionResult { selected_ids }))
        }
        Err(inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted) => {
            Ok(None)
        }
        Err(e) => Err(e.into()),
    }
}

/// Show a multi-select UI for choosing which providers to install hooks for.
/// Only detected providers can be selected.
/// Returns None if the user cancels.
pub fn select_providers(registry: &ProviderRegistry) -> Result<Option<SelectionResult>> {
    run_selection(
        registry,
        "Select AI coding tools to install hooks for:",
        "No supported AI coding tools detected on this system.",
        "No tools detected. Please install a supported AI coding tool first.",
        true, // show install hints
    )
}

/// Show a multi-select UI for choosing which providers to uninstall hooks from.
/// Only detected providers can be selected.
/// Returns None if the user cancels.
pub fn select_providers_for_uninstall(registry: &ProviderRegistry) -> Result<Option<SelectionResult>> {
    run_selection(
        registry,
        "Select AI coding tools to uninstall hooks from:",
        "No supported AI coding tools detected on this system.",
        "No tools detected. Nothing to uninstall.",
        false, // don't show install hints for uninstall
    )
}
