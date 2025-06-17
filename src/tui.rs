//! Simple TUI implementation with minimal components
//! 
//! Provides three basic UI elements:
//! 1. Input box - for search queries
//! 2. Results box - for displaying search results
//! 3. Status bar - for basic help information

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::{
    io::{stdout, Result, Stdout},
    time::Duration,
};

/// Simple TUI application with three basic components
pub struct TuiApp {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    search_input: String,
    search_results: Vec<String>,
    should_quit: bool,
}

impl TuiApp {
    /// Create new TUI application
    pub async fn new(_search_path: &str) -> Result<Self> {
        // Setup terminal with error handling
        enable_raw_mode().map_err(|e| {
            eprintln!("Failed to enable raw mode: {}", e);
            eprintln!("Note: TUI mode requires a proper terminal environment");
            e
        })?;
        
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture).map_err(|e| {
            let _ = disable_raw_mode(); // Cleanup on failure
            eprintln!("Failed to setup terminal: {}", e);
            e
        })?;
        
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend).map_err(|e| {
            let _ = disable_raw_mode(); // Cleanup on failure
            eprintln!("Failed to create terminal: {}", e);
            e
        })?;

        Ok(TuiApp {
            terminal,
            search_input: String::new(),
            search_results: Vec::new(),
            should_quit: false,
        })
    }

    /// Run the TUI application
    pub async fn run(&mut self) -> Result<()> {
        // Initial render
        self.draw()?;

        // Main event loop
        loop {
            // Handle input events
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key_event(key);
                }
            }

            // Check if we should quit
            if self.should_quit {
                break;
            }

            // Re-render
            self.draw()?;

            // Small delay to prevent busy waiting
            std::thread::sleep(Duration::from_millis(16));
        }

        self.cleanup()?;
        Ok(())
    }

    /// Handle keyboard input
    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            // Quit commands
            KeyCode::Esc | KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }

            // Text input
            KeyCode::Char(c) => {
                self.search_input.push(c);
                self.update_search_results();
            }
            KeyCode::Backspace => {
                self.search_input.pop();
                self.update_search_results();
            }

            // Enter to trigger search
            KeyCode::Enter => {
                self.update_search_results();
            }

            _ => {}
        }
    }

    /// Update search results based on current input
    fn update_search_results(&mut self) {
        self.search_results.clear();
        
        if !self.search_input.is_empty() {
            // Add some mock results for demonstration
            for i in 1..=5 {
                self.search_results.push(format!(
                    "src/file_{}.rs:{}:Mock result for '{}'",
                    i, i * 10, self.search_input
                ));
            }
        }
    }

    /// Draw the UI
    fn draw(&mut self) -> Result<()> {
        let search_input = self.search_input.clone();
        let search_results = self.search_results.clone();
        
        self.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Input box
                    Constraint::Min(1),    // Results box
                    Constraint::Length(3), // Status bar
                ])
                .split(f.size());

            // 1. Input box
            render_input_box(f, chunks[0], &search_input);

            // 2. Results box
            render_results_box(f, chunks[1], &search_results);

            // 3. Status bar
            render_status_bar(f, chunks[2]);
        })?;
        Ok(())
    }


    /// Cleanup terminal on exit
    fn cleanup(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

/// Render the input box
fn render_input_box(f: &mut Frame, area: ratatui::layout::Rect, search_input: &str) {
    let input = Paragraph::new(search_input)
        .block(Block::default().borders(Borders::ALL).title("Search Input"))
        .style(Style::default().fg(Color::White));
    f.render_widget(input, area);
}

/// Render the results box
fn render_results_box(f: &mut Frame, area: ratatui::layout::Rect, search_results: &[String]) {
    let items: Vec<ListItem> = search_results
        .iter()
        .map(|result| ListItem::new(result.as_str()))
        .collect();

    let results_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Search Results"))
        .style(Style::default().fg(Color::White));

    f.render_widget(results_list, area);
}

/// Render the status bar
fn render_status_bar(f: &mut Frame, area: ratatui::layout::Rect) {
    let help_text = "Type to search | Enter: Execute | Esc/Ctrl+C: Quit";
    let status = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .style(Style::default().fg(Color::Gray));
    f.render_widget(status, area);
}