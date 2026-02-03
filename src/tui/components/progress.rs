//! Progress display component for showing step-by-step status updates.
//!
//! This is a simple inline progress display that prints directly to stdout.

use colored::Colorize;

/// Handle for sending progress updates
#[derive(Clone)]
pub struct ProgressHandle {
    _title: String,
}

impl ProgressHandle {
    /// Add a new step (prints the message with a spinner)
    pub fn step(&self, message: impl Into<String>) {
        println!("{} {}", "→".blue(), message.into());
    }

    /// Mark the current step as completed without starting a new one
    pub fn complete(&self) {
        // No-op for inline mode
    }

    /// Mark the current step as failed
    pub fn fail(&self, error: impl Into<String>) {
        println!("{} {}", "✗".red(), error.into().red());
    }

    /// Update the current step's message
    pub fn update(&self, message: impl Into<String>) {
        println!("  {}", message.into());
    }

    /// Close the progress display
    pub fn done(&self) {
        // No-op for inline mode
    }
}

/// A simple progress display that prints steps inline
pub struct Progress {
    title: String,
}

impl Progress {
    /// Creates a new progress display with the given title
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
        }
    }

    /// Starts the progress display and returns a handle for sending updates.
    ///
    /// # Errors
    ///
    /// This implementation never fails.
    pub fn start(self) -> anyhow::Result<ProgressHandle> {
        println!("\n{}", self.title.cyan().bold());
        Ok(ProgressHandle { _title: self.title })
    }
}
