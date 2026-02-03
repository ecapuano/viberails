//! Reusable TUI components for user prompts.

mod config_view;
mod message;
mod multiselect;
mod progress;
mod select;
mod text_input;

pub use config_view::{ConfigEntry, ConfigView};
pub use message::{Message, MessageStyle};
pub use multiselect::{MultiSelect, MultiSelectItem};
pub use progress::{Progress, ProgressHandle};
pub use select::{Select, SelectItem};
pub use text_input::TextInput;

use anyhow::Result;

/// Result of input validation.
#[derive(Debug, Clone)]
pub enum ValidationResult {
    /// Input is valid
    Valid,
    /// Input is invalid with an error message
    Invalid(String),
}

impl ValidationResult {
    /// Returns true if the validation passed.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }
}

/// Result type for prompt operations.
///
/// - `Ok(Some(value))` - User submitted a value
/// - `Ok(None)` - User cancelled (Escape or Ctrl+C)
/// - `Err(_)` - An error occurred
pub type PromptResult<T> = Result<Option<T>>;
