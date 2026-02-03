//! Theme configuration for consistent styling across TUI components.

use ratatui::style::{Color, Modifier, Style};

/// Theme configuration for TUI components.
///
/// Provides consistent colors and styles across all prompt types.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Style for titles and headers
    pub title: Style,
    /// Style for selected/highlighted items
    pub selected: Style,
    /// Style for normal, unselected items
    pub unselected: Style,
    /// Style for disabled/unavailable items
    pub disabled: Style,
    /// Style for the input cursor
    pub cursor: Style,
    /// Style for help text at the bottom
    pub help: Style,
    /// Style for error messages
    pub error: Style,
    /// Style for success indicators
    pub success: Style,
    /// Style for borders
    pub border: Style,
    /// Style for the selection indicator (arrow/checkbox)
    pub indicator: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            title: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            selected: Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            unselected: Style::default().fg(Color::White),
            disabled: Style::default().fg(Color::DarkGray),
            cursor: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            help: Style::default().fg(Color::DarkGray),
            error: Style::default().fg(Color::Red),
            success: Style::default().fg(Color::Green),
            border: Style::default().fg(Color::Blue),
            indicator: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        }
    }
}

impl Theme {
    /// Creates a new theme with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
