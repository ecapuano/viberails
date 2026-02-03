//! Text input component with validation support.

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use tui_input::{Input, backend::crossterm::EventHandler};

use super::{PromptResult, ValidationResult};
use crate::tui::{TerminalApp, theme::Theme};

/// A text input prompt with optional validation.
pub struct TextInput<'a, V>
where
    V: Fn(&str) -> ValidationResult,
{
    title: &'a str,
    help_message: Option<&'a str>,
    validator: Option<V>,
    theme: Theme,
}

impl<'a> TextInput<'a, fn(&str) -> ValidationResult> {
    /// Creates a new text input prompt with the given title.
    #[must_use]
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            help_message: None,
            validator: None,
            theme: Theme::default(),
        }
    }
}

impl<'a, V> TextInput<'a, V>
where
    V: Fn(&str) -> ValidationResult,
{
    /// Sets the help message displayed below the input.
    #[must_use]
    pub fn with_help_message(mut self, message: &'a str) -> Self {
        self.help_message = Some(message);
        self
    }

    /// Sets the validator function for the input.
    #[must_use]
    pub fn with_validator<NewV>(self, validator: NewV) -> TextInput<'a, NewV>
    where
        NewV: Fn(&str) -> ValidationResult,
    {
        TextInput {
            title: self.title,
            help_message: self.help_message,
            validator: Some(validator),
            theme: self.theme,
        }
    }

    /// Sets a custom theme for the input.
    #[must_use]
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Runs the text input prompt and returns the user's input.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(text))` - User submitted text
    /// - `Ok(None)` - User cancelled with Escape
    /// - `Err(_)` - Terminal error occurred
    pub fn prompt(self) -> PromptResult<String> {
        let mut app = TerminalApp::new()?;
        let mut input = Input::default();
        let mut error_message: Option<String> = None;

        loop {
            app.terminal().draw(|frame| {
                self.render(frame, &input, error_message.as_deref());
            })?;

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Enter => {
                        let value = input.value().to_string();
                        if let Some(ref validator) = self.validator {
                            match validator(&value) {
                                ValidationResult::Valid => return Ok(Some(value)),
                                ValidationResult::Invalid(msg) => {
                                    error_message = Some(msg);
                                }
                            }
                        } else {
                            return Ok(Some(value));
                        }
                    }
                    KeyCode::Esc => return Ok(None),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(None);
                    }
                    _ => {
                        input.handle_event(&Event::Key(key));
                        error_message = None;
                    }
                }
            }
        }
    }

    #[allow(
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects,
        clippy::cast_possible_truncation
    )]
    fn render(&self, frame: &mut Frame, input: &Input, error_message: Option<&str>) {
        let area = centered_rect(60, 9, frame.area());

        frame.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border);

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner_area);

        // Render the prompt/title text above the input
        let title_line = Line::from(Span::styled(self.title, self.theme.title));
        frame.render_widget(Paragraph::new(title_line), chunks[0]);

        let input_width = chunks[1].width.saturating_sub(2);
        let scroll = calculate_scroll(input.visual_cursor(), input_width as usize);

        let input_widget = Paragraph::new(input.value())
            .scroll((0, scroll as u16))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(self.theme.border),
            );
        frame.render_widget(input_widget, chunks[1]);

        let cursor_x = chunks[1].x + 1 + (input.visual_cursor() - scroll) as u16;
        let cursor_y = chunks[1].y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));

        if let Some(err) = error_message {
            let error_line = Line::from(Span::styled(err, self.theme.error));
            frame.render_widget(Paragraph::new(error_line), chunks[2]);
        } else if let Some(help) = self.help_message {
            let help_line = Line::from(Span::styled(help, self.theme.help));
            frame.render_widget(Paragraph::new(help_line), chunks[2]);
        } else {
            let default_help = "Enter to submit, Esc to cancel";
            let help_line = Line::from(Span::styled(default_help, self.theme.help));
            frame.render_widget(Paragraph::new(help_line), chunks[2]);
        }
    }
}

fn calculate_scroll(cursor: usize, width: usize) -> usize {
    if cursor >= width {
        cursor.saturating_sub(width).saturating_add(1)
    } else {
        0
    }
}

#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .split(area);

    let horizontal = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1]);

    horizontal[1]
}
