//! Single selection component.

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
};

use super::PromptResult;
use crate::tui::{TerminalApp, theme::Theme};

/// An item in a selection list.
#[derive(Debug, Clone)]
pub struct SelectItem<T> {
    /// The value returned when this item is selected
    pub value: T,
    /// The label displayed to the user
    pub label: String,
    /// Whether this item can be selected
    pub enabled: bool,
}

impl<T> SelectItem<T> {
    /// Creates a new enabled select item.
    #[must_use]
    pub fn new(value: T, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
            enabled: true,
        }
    }

    /// Creates a new disabled select item.
    #[must_use]
    #[allow(dead_code)]
    pub fn disabled(value: T, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
            enabled: false,
        }
    }
}

/// A single selection prompt.
pub struct Select<'a, T> {
    title: &'a str,
    subtitle: Option<&'a str>,
    items: Vec<SelectItem<T>>,
    help_message: Option<&'a str>,
    starting_index: usize,
    theme: Theme,
}

impl<'a, T> Select<'a, T> {
    /// Creates a new select prompt with the given title and items.
    #[must_use]
    pub fn new(title: &'a str, items: Vec<SelectItem<T>>) -> Self {
        Self {
            title,
            subtitle: None,
            items,
            help_message: None,
            starting_index: 0,
            theme: Theme::default(),
        }
    }

    /// Sets the subtitle displayed in the top-right corner.
    #[must_use]
    pub fn with_subtitle(mut self, subtitle: &'a str) -> Self {
        self.subtitle = Some(subtitle);
        self
    }

    /// Sets the help message displayed below the list.
    #[must_use]
    pub fn with_help_message(mut self, message: &'a str) -> Self {
        self.help_message = Some(message);
        self
    }

    /// Sets the initial cursor position.
    #[must_use]
    pub fn with_starting_cursor(mut self, index: usize) -> Self {
        self.starting_index = index;
        self
    }

    /// Sets a custom theme for the select.
    #[must_use]
    #[allow(dead_code)]
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Runs the select prompt and returns the selected item's value.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(value))` - User selected an item
    /// - `Ok(None)` - User cancelled with Escape
    /// - `Err(_)` - Terminal error occurred
    pub fn prompt(self) -> PromptResult<T> {
        if self.items.is_empty() {
            return Ok(None);
        }

        let mut app = TerminalApp::new()?;
        let mut state = ListState::default();
        state.select(Some(
            self.starting_index.min(self.items.len().saturating_sub(1)),
        ));

        loop {
            app.terminal().draw(|frame| {
                self.render(frame, &mut state);
            })?;

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.move_cursor_up(&mut state);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.move_cursor_down(&mut state);
                    }
                    KeyCode::Enter => {
                        if let Some(idx) = state.selected()
                            && let Some(item) = self.items.get(idx)
                            && item.enabled
                        {
                            let items = self.items;
                            return Ok(items.into_iter().nth(idx).map(|i| i.value));
                        }
                    }
                    KeyCode::Esc => return Ok(None),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(None);
                    }
                    _ => {}
                }
            }
        }
    }

    fn move_cursor_up(&self, state: &mut ListState) {
        let current = state.selected().unwrap_or(0);
        let mut new_idx = current;

        for _ in 0..self.items.len() {
            new_idx = if new_idx == 0 {
                self.items.len().saturating_sub(1)
            } else {
                new_idx.saturating_sub(1)
            };

            if let Some(item) = self.items.get(new_idx)
                && item.enabled
            {
                state.select(Some(new_idx));
                return;
            }
        }
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn move_cursor_down(&self, state: &mut ListState) {
        let current = state.selected().unwrap_or(0);
        let mut new_idx = current;

        for _ in 0..self.items.len() {
            new_idx = (new_idx + 1) % self.items.len();

            if let Some(item) = self.items.get(new_idx)
                && item.enabled
            {
                state.select(Some(new_idx));
                return;
            }
        }
    }

    #[allow(
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects,
        clippy::cast_possible_truncation
    )]
    fn render(&self, frame: &mut Frame, state: &mut ListState) {
        // Cap list height to max 15 items visible (plus border + help) to ensure scrolling works
        let max_visible_items: u16 = 15;
        let list_height = (self.items.len() as u16).min(max_visible_items) + 4; // items + border + help
        let height = list_height.min(frame.area().height.saturating_sub(2));
        let area = centered_rect(60, height, frame.area());

        frame.render_widget(Clear, area);

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border)
            .title(Span::styled(self.title, self.theme.title));

        if let Some(subtitle) = self.subtitle {
            block = block.title_top(
                Line::from(Span::styled(subtitle, self.theme.help)).alignment(Alignment::Right),
            );
        }

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        let chunks =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner_area);

        let list_items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let is_selected = state.selected() == Some(idx);
                let prefix = if is_selected { "> " } else { "  " };

                let style = if !item.enabled {
                    self.theme.disabled
                } else if is_selected {
                    self.theme.selected
                } else {
                    self.theme.unselected
                };

                ListItem::new(Line::from(Span::styled(
                    format!("{prefix}{}", item.label),
                    style,
                )))
            })
            .collect();

        let list = List::new(list_items).scroll_padding(1);
        frame.render_stateful_widget(list, chunks[0], state);

        let help_text = self
            .help_message
            .unwrap_or("↑↓ navigate, Enter select, Esc cancel");
        let help_line = Line::from(Span::styled(help_text, self.theme.help));
        frame.render_widget(help_line, chunks[1]);
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
