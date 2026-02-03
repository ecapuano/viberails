//! High-level prompt functions for common use cases.
//!
//! These functions provide simple, drop-in replacements for the previous inquire-based prompts.

use std::io::{self, Write};

use colored::Colorize;

use super::components::{
    Message, MessageStyle, MultiSelect, MultiSelectItem, PromptResult, Select, SelectItem,
    ValidationResult,
};

/// Creates a simple inline text input prompt.
///
/// This version prints directly to stdout without using the alternate screen,
/// making it more compatible with various terminal environments.
///
/// # Arguments
///
/// * `title` - The prompt title
/// * `_help` - Optional help message (currently unused in inline mode)
/// * `validator` - Optional validation function
///
/// # Returns
///
/// - `Ok(Some(text))` - User entered text
/// - `Ok(None)` - User cancelled (empty input)
/// - `Err(_)` - IO error
///
/// # Example
///
/// ```ignore
/// let name = text_prompt(
///     "Enter your name:",
///     Some("Press Enter to confirm"),
///     Some(|s: &str| {
///         if s.is_empty() {
///             ValidationResult::Invalid("Name cannot be empty".into())
///         } else {
///             ValidationResult::Valid
///         }
///     }),
/// )?;
/// ```
pub fn text_prompt<V>(
    title: &str,
    _help: Option<&str>,
    validator: Option<&V>,
) -> PromptResult<String>
where
    V: Fn(&str) -> ValidationResult,
{
    loop {
        // Print the prompt
        print!("{} ", title.cyan().bold());
        io::stdout().flush()?;

        // Read input
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        // Check if empty (treat as cancel if no validator)
        if input.is_empty() && validator.is_none() {
            return Ok(None);
        }

        // Validate if validator provided
        if let Some(v) = validator {
            match v(&input) {
                ValidationResult::Valid => return Ok(Some(input)),
                ValidationResult::Invalid(msg) => {
                    println!("{} {}", "Error:".red(), msg);
                }
            }
        } else {
            return Ok(Some(input));
        }
    }
}

/// Creates a single-selection prompt from a list of string options.
///
/// # Arguments
///
/// * `title` - The prompt title
/// * `options` - List of string options to choose from
/// * `help` - Optional help message
///
/// # Returns
///
/// - `Ok(Some(index))` - Index of the selected option
/// - `Ok(None)` - User cancelled
/// - `Err(_)` - Terminal error
///
/// # Example
///
/// ```ignore
/// let choice = select_prompt(
///     "Select an option:",
///     vec!["Option 1", "Option 2", "Option 3"],
///     Some("Use arrow keys to navigate"),
/// )?;
/// ```
pub fn select_prompt(title: &str, options: Vec<&str>, help: Option<&str>) -> PromptResult<usize> {
    select_prompt_with_subtitle(title, options, help, None)
}

/// Creates a single-selection prompt with an optional subtitle in the top-right corner.
///
/// # Arguments
///
/// * `title` - The prompt title
/// * `options` - List of string options to choose from
/// * `help` - Optional help message
/// * `subtitle` - Optional subtitle displayed in the top-right corner (e.g., version)
///
/// # Returns
///
/// - `Ok(Some(index))` - Index of the selected option
/// - `Ok(None)` - User cancelled
/// - `Err(_)` - Terminal error
pub fn select_prompt_with_subtitle(
    title: &str,
    options: Vec<&str>,
    help: Option<&str>,
    subtitle: Option<&str>,
) -> PromptResult<usize> {
    let items: Vec<SelectItem<usize>> = options
        .into_iter()
        .enumerate()
        .map(|(idx, label)| SelectItem::new(idx, label))
        .collect();

    let mut prompt = Select::new(title, items);

    if let Some(h) = help {
        prompt = prompt.with_help_message(h);
    }

    if let Some(s) = subtitle {
        prompt = prompt.with_subtitle(s);
    }

    prompt.prompt()
}

/// Creates a multi-selection prompt with custom items and optional validation.
///
/// # Arguments
///
/// * `title` - The prompt title
/// * `items` - List of multi-select items
/// * `help` - Optional help message
/// * `validator` - Optional validation function
///
/// # Returns
///
/// - `Ok(Some(values))` - The selected items' values
/// - `Ok(None)` - User cancelled
/// - `Err(_)` - Terminal error
///
/// # Example
///
/// ```ignore
/// let items = vec![
///     MultiSelectItem::new("git", "Git").selected(true),
///     MultiSelectItem::new("docker", "Docker"),
///     MultiSelectItem::disabled("kubernetes", "Kubernetes (not installed)"),
/// ];
///
/// let selected = multiselect_prompt(
///     "Select tools:",
///     items,
///     Some("Space to toggle, Enter to confirm"),
///     Some(|selections: &[&&str]| {
///         if selections.is_empty() {
///             ValidationResult::Invalid("Select at least one".into())
///         } else {
///             ValidationResult::Valid
///         }
///     }),
/// )?;
/// ```
pub fn multiselect_prompt<T, V>(
    title: &str,
    items: Vec<MultiSelectItem<T>>,
    help: Option<&str>,
    validator: Option<V>,
) -> PromptResult<Vec<T>>
where
    V: Fn(&[&T]) -> ValidationResult,
{
    let mut prompt = MultiSelect::new(title, items);

    if let Some(h) = help {
        prompt = prompt.with_help_message(h);
    }

    if let Some(v) = validator {
        prompt.with_validator(v).prompt()
    } else {
        prompt.prompt()
    }
}

/// Displays a message to the user and waits for acknowledgment.
///
/// # Arguments
///
/// * `title` - The message box title
/// * `message` - The message to display
/// * `style` - The style of the message (Info, Error, or Success)
///
/// # Returns
///
/// - `Ok(())` - User acknowledged the message
/// - `Err(_)` - Terminal error
///
/// # Example
///
/// ```ignore
/// message_prompt("Error", "Not logged in", MessageStyle::Error)?;
/// ```
pub fn message_prompt(title: &str, message: &str, style: MessageStyle) -> io::Result<()> {
    Message::new(title, message).with_style(style).show()
}
