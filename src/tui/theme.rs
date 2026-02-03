//! Theme configuration for consistent styling across TUI components.

use ratatui::style::{Color, Modifier, Style};

// Viberails brand colors (from https://www.viberails.io/)
// Gradient: cyan (#22d3ee) -> blue (#3b82f6) -> pink (#db2777)
const VR_CYAN: Color = Color::Rgb(34, 211, 238); // #22d3ee
const VR_BLUE: Color = Color::Rgb(59, 130, 246); // #3b82f6
const VR_PINK: Color = Color::Rgb(219, 39, 119); // #db2777
const VR_BLUE_LIGHT: Color = Color::Rgb(96, 165, 250); // #60a5fa (--vr-blue-6)
const VR_GRAY: Color = Color::Rgb(156, 163, 175); // rgb(156 163 175)
const VR_GRAY_LIGHT: Color = Color::Rgb(209, 213, 219); // rgb(209 213 219)

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
            title: Style::default().fg(VR_CYAN).add_modifier(Modifier::BOLD),
            selected: Style::default().fg(VR_PINK).add_modifier(Modifier::BOLD),
            unselected: Style::default().fg(VR_GRAY_LIGHT),
            disabled: Style::default().fg(VR_GRAY),
            cursor: Style::default().fg(VR_PINK).add_modifier(Modifier::BOLD),
            help: Style::default().fg(VR_GRAY),
            error: Style::default().fg(VR_PINK),
            success: Style::default().fg(VR_CYAN),
            border: Style::default().fg(VR_BLUE),
            indicator: Style::default()
                .fg(VR_BLUE_LIGHT)
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
