use arboard::Clipboard;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::thread;
use std::time::Duration;

use crate::{
    backend::{BackendEvent, SearchBackend, UserCommand},
    tui_state::{StatusSpan, TuiAction, TuiInput, TuiState},
};

/// New TUI Application using event-based architecture
/// This only handles UI rendering and user input, all business logic is in SearchBackend
pub struct TuiApp {
    state: TuiState,
    command_sender: std::sync::mpsc::Sender<UserCommand>,
    event_receiver: std::sync::mpsc::Receiver<BackendEvent>,
    backend_thread: Option<thread::JoinHandle<()>>,
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new(false, true).expect("Failed to create default TuiApp")
    }
}

impl TuiApp {
    /// Create new TUI app with backend
    pub fn new(verbose: bool, respect_gitignore: bool) -> anyhow::Result<Self> {
        let (mut backend, command_sender, event_receiver) =
            SearchBackend::new(verbose, respect_gitignore);

        // Start backend in separate thread
        let backend_thread = thread::spawn(move || {
            if let Err(e) = backend.run() {
                eprintln!("Backend error: {}", e);
            }
        });

        // Give backend a moment to initialize
        thread::sleep(Duration::from_millis(50));

        Ok(Self {
            state: TuiState::new(),
            command_sender,
            event_receiver,
            backend_thread: Some(backend_thread),
        })
    }

    /// Initialize TUI with directory indexing
    pub fn initialize(&mut self, directory: std::path::PathBuf) -> anyhow::Result<()> {
        self.send_command(UserCommand::StartIndexing { directory })?;
        Ok(())
    }

    /// Enable file watching
    pub fn enable_file_watching(&mut self) -> anyhow::Result<()> {
        self.state.watch_enabled = true;
        self.send_command(UserCommand::EnableFileWatching)?;
        Ok(())
    }

    /// Send command to backend
    fn send_command(&mut self, command: UserCommand) -> anyhow::Result<()> {
        self.command_sender
            .send(command)
            .map_err(|e| anyhow::anyhow!("Failed to send command: {}", e))?;
        Ok(())
    }

    /// Process backend events (non-blocking)
    fn process_backend_events(&mut self) {
        // Process up to 5 events per frame to maintain responsiveness
        for _ in 0..5 {
            match self.event_receiver.try_recv() {
                Ok(event) => {
                    self.state.apply_backend_event(event);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // No more events available
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // Backend disconnected
                    self.state.should_quit = true;
                    break;
                }
            }
        }
    }

    /// Handle keyboard input
    fn handle_key_event(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        let input = match key.code {
            KeyCode::Esc => TuiInput::Quit,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => TuiInput::Quit,
            KeyCode::Char('?') => TuiInput::ToggleHelp,
            KeyCode::Up | KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                TuiInput::NavigateUp
            }
            KeyCode::Down | KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                TuiInput::NavigateDown
            }
            KeyCode::Enter => TuiInput::Select,
            KeyCode::Backspace => TuiInput::Backspace,
            KeyCode::Char(c) => TuiInput::TypeChar(c),
            _ => return Ok(()), // Ignore other keys
        };

        let actions = self.state.handle_input(input);

        for action in actions {
            match action {
                TuiAction::Quit => {
                    self.send_command(UserCommand::Quit)?;
                }
                TuiAction::Search { query, mode } => {
                    self.send_command(UserCommand::Search { query, mode })?;
                }
                TuiAction::CopyToClipboard { text } => {
                    self.copy_to_clipboard(&text)?;
                }
            }
        }

        Ok(())
    }

    /// Copy text to clipboard
    fn copy_to_clipboard(&mut self, text: &str) -> anyhow::Result<()> {
        // Temporarily disable raw mode for clipboard operation
        disable_raw_mode()?;

        let result =
            Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text.to_string()));

        // Re-enable raw mode
        enable_raw_mode()?;

        match result {
            Ok(_) => {
                self.state.status_message = format!("Copied: {}", text);
            }
            Err(e) => {
                self.state.status_message = format!("Failed to copy: {}", e);
            }
        }

        Ok(())
    }

    /// Main run loop
    pub fn run(
        &mut self,
        directory: std::path::PathBuf,
        watch_enabled: bool,
    ) -> anyhow::Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Initialize with directory
        self.initialize(directory)?;

        // Enable file watching if requested
        if watch_enabled {
            self.enable_file_watching()?;
        }

        // Main event loop
        loop {
            // Process backend events
            self.process_backend_events();

            // Render UI
            terminal.draw(|f| self.ui(f))?;

            // Check for quit
            if self.state.should_quit {
                break;
            }

            // Handle user input with timeout
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key_event(key)?;
                }
            }
        }

        // Cleanup
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        Ok(())
    }

    /// Initialize TUI for testing (no terminal setup)
    pub fn initialize_for_testing(&mut self, directory: std::path::PathBuf) -> anyhow::Result<()> {
        self.initialize(directory)?;
        Ok(())
    }

    /// For testing: get current state
    pub fn get_state(&self) -> &TuiState {
        &self.state
    }

    /// For testing: simulate input
    pub fn simulate_input(&mut self, input: TuiInput) -> anyhow::Result<()> {
        let actions = self.state.handle_input(input);

        for action in actions {
            match action {
                TuiAction::Quit => {
                    self.send_command(UserCommand::Quit)?;
                }
                TuiAction::Search { query, mode } => {
                    self.send_command(UserCommand::Search { query, mode })?;
                }
                TuiAction::CopyToClipboard { text: _ } => {
                    // Skip clipboard in testing
                }
            }
        }

        Ok(())
    }

    /// UI rendering function
    fn ui(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(3), // Search box
                    Constraint::Min(0),    // Results
                    Constraint::Length(3), // Status bar
                ]
                .as_ref(),
            )
            .split(f.size());

        // Render search box
        self.render_search_box(f, chunks[0]);

        // Render results or help
        if self.state.show_help {
            self.render_help_popup(f, chunks[1]);
        } else {
            self.render_results(f, chunks[1]);
        }

        // Render status bar
        self.render_status(f, chunks[2]);
    }

    /// Render search box
    fn render_search_box(&self, f: &mut Frame, area: Rect) {
        let mode_info = self.state.get_mode_info();

        let search_block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", mode_info))
            .style(Style::default().fg(Color::Cyan));

        let search_text = Paragraph::new(self.state.query.as_str())
            .block(search_block)
            .style(Style::default().fg(Color::White));

        f.render_widget(search_text, area);
    }

    /// Render search results
    fn render_results(&mut self, f: &mut Frame, area: Rect) {
        let title = if self.state.is_indexing {
            format!(
                " Results ({} found) (indexing...) ",
                self.state.current_results.len()
            )
        } else {
            format!(" Results ({} found) ", self.state.current_results.len())
        };

        let results_block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().fg(Color::Green));

        let items: Vec<ListItem> = self
            .state
            .current_results
            .iter()
            .enumerate()
            .map(|(i, result)| {
                let content = format!(
                    "{} {}:{}:{}",
                    result.symbol.name,
                    result
                        .symbol
                        .file
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy(),
                    result.symbol.line,
                    result.symbol.column
                );

                let style = if i == self.state.selected_index {
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(content).style(style)
            })
            .collect();

        let results_list = List::new(items)
            .block(results_block)
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        let mut list_state = ListState::default();
        if !self.state.current_results.is_empty() {
            list_state.select(Some(self.state.selected_index));
        }

        f.render_stateful_widget(results_list, area, &mut list_state);
    }

    /// Render help popup
    fn render_help_popup(&self, f: &mut Frame, area: Rect) {
        let help_text = vec![
            Line::from("🔍 Search Modes:"),
            Line::from(""),
            Line::from("  🔍 Default - Content search"),
            Line::from("  🏷️ #symbol - Search symbols only"),
            Line::from("  📁 >file - Search files and directories"),
            Line::from("  📁 >dir/ - Search directories only (with trailing /)"),
            Line::from("  🔧 /regex - Regular expression search"),
            Line::from(""),
            Line::from("⌨️ Key Bindings:"),
            Line::from(""),
            Line::from("  ↑/↓ or Ctrl+P/N - Navigate results"),
            Line::from("  Enter - Copy location to clipboard"),
            Line::from("  ? - Toggle this help"),
            Line::from("  Esc/Ctrl+C - Quit"),
            Line::from(""),
            Line::from("Press ? to close help"),
        ];

        let help_paragraph = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title(" Help "))
            .style(Style::default().fg(Color::White));

        // Create a popup area (centered)
        let popup_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(15),
                    Constraint::Percentage(70),
                    Constraint::Percentage(15),
                ]
                .as_ref(),
            )
            .split(area)[1];

        let popup_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(10),
                    Constraint::Percentage(80),
                    Constraint::Percentage(10),
                ]
                .as_ref(),
            )
            .split(popup_area)[1];

        f.render_widget(Clear, popup_area);
        f.render_widget(help_paragraph, popup_area);
    }

    /// Render status bar
    fn render_status(&self, f: &mut Frame, area: Rect) {
        let status_spans = self.state.get_status_info();

        let spans: Vec<Span> = status_spans
            .into_iter()
            .map(|span| match span {
                StatusSpan::Label(text) => Span::styled(text, Style::default().fg(Color::Blue)),
                StatusSpan::Text(text) => Span::styled(text, Style::default().fg(Color::White)),
                StatusSpan::Success(text) => Span::styled(text, Style::default().fg(Color::Green)),
                StatusSpan::Error(text) => Span::styled(text, Style::default().fg(Color::Red)),
            })
            .collect();

        let status_paragraph = Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::White));

        f.render_widget(status_paragraph, area);
    }
}

impl Drop for TuiApp {
    fn drop(&mut self) {
        // Send quit command to clean up backend
        let _ = self.command_sender.send(UserCommand::Quit);

        // Wait for backend thread to finish
        if let Some(handle) = self.backend_thread.take() {
            let _ = handle.join();
        }
    }
}

// Public API functions for backward compatibility
pub fn run_tui(
    directory: std::path::PathBuf,
    verbose: bool,
    respect_gitignore: bool,
) -> anyhow::Result<()> {
    run_tui_with_watch(directory, false, verbose, respect_gitignore)
}

pub fn run_tui_with_watch(
    directory: std::path::PathBuf,
    watch_enabled: bool,
    verbose: bool,
    respect_gitignore: bool,
) -> anyhow::Result<()> {
    let mut app = TuiApp::new(verbose, respect_gitignore)?;
    app.run(directory, watch_enabled)
}
