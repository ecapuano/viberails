//! Professional terminal UI components built on ratatui.
//!
//! This module provides reusable prompt components for user interaction:
//! - `TextInput` - Text input with validation
//! - `Select` - Single selection from a list
//! - `MultiSelect` - Multiple selection with toggle

mod app;
pub mod components;
mod prompts;
mod theme;

pub use app::TerminalApp;
pub use components::{
    ConfigEntry, ConfigView, MessageStyle, Progress, ProgressHandle, PromptResult, ValidationResult,
};
pub use prompts::{
    message_prompt, multiselect_prompt, select_prompt, select_prompt_with_subtitle, text_prompt,
};
pub use theme::Theme;
