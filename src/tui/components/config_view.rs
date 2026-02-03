//! Configuration display component.

use colored::{Color, Colorize};

/// A configuration entry to display.
#[derive(Debug, Clone)]
pub struct ConfigEntry {
    /// The label/key for this configuration
    pub label: String,
    /// The value to display
    pub value: String,
}

impl ConfigEntry {
    /// Creates a new configuration entry.
    #[must_use]
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }

    /// Creates a configuration entry for a boolean value.
    #[must_use]
    pub fn bool(label: impl Into<String>, value: bool) -> Self {
        Self {
            label: label.into(),
            value: if value {
                "enabled".to_string()
            } else {
                "disabled".to_string()
            },
        }
    }
}

/// A read-only configuration display component that prints inline to stdout.
///
/// Uses colors matching the TUI theme for consistency.
pub struct ConfigView<'a> {
    title: &'a str,
    entries: Vec<ConfigEntry>,
}

impl<'a> ConfigView<'a> {
    /// Creates a new configuration view with the given title and entries.
    #[must_use]
    pub fn new(title: &'a str, entries: Vec<ConfigEntry>) -> Self {
        Self { title, entries }
    }

    /// Displays the configuration to stdout.
    ///
    /// The output remains visible after the function returns, allowing
    /// users to copy values like URLs.
    pub fn print(&self) {
        if self.entries.is_empty() {
            return;
        }

        // Calculate the maximum label width for alignment
        let max_label_width = self
            .entries
            .iter()
            .map(|e| e.label.len())
            .max()
            .unwrap_or(0);

        // Print title (cyan bold, matching theme.title)
        println!();
        println!("{}", self.title.color(Color::Cyan).bold());
        println!("{}", "─".repeat(self.title.len()).color(Color::Blue));
        println!();

        // Print each entry
        for entry in &self.entries {
            let padding = max_label_width.saturating_sub(entry.label.len());
            println!(
                "  {}{} {} {}",
                entry.label.color(Color::Cyan).bold(),
                " ".repeat(padding),
                ":".color(Color::BrightBlack),
                entry.value.color(Color::White)
            );
        }

        println!();
    }
}
