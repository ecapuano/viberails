//! Terminal application wrapper with RAII setup/teardown.

use std::io::{self, Stdout};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

/// Type alias for the terminal with crossterm backend.
pub type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

/// RAII wrapper for terminal setup and teardown.
///
/// Automatically enables raw mode and alternate screen on creation,
/// and restores the terminal on drop (even on panic).
pub struct TerminalApp {
    terminal: TuiTerminal,
}

impl TerminalApp {
    /// Creates a new terminal application, setting up raw mode and alternate screen.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal setup fails.
    pub fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    /// Returns a mutable reference to the underlying terminal.
    pub fn terminal(&mut self) -> &mut TuiTerminal {
        &mut self.terminal
    }
}

impl Drop for TerminalApp {
    fn drop(&mut self) {
        // Attempt to restore terminal state, ignoring errors during cleanup
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
    }
}
