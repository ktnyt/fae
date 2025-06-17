//! Simple TUI implementation with minimal components
//! 
//! Provides three basic UI elements:
//! 1. Input box - for search queries
//! 2. Results box - for displaying search results
//! 3. Status bar - for basic help information
//!
//! Internal state includes:
//! - Input string for search queries
//! - Search results array with navigation
//! - Cursor position for result selection
//! - Toast display state and content

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
    time::{Duration, Instant},
};

/// Toast notification state and content
#[derive(Clone, Debug)]
pub struct ToastState {
    pub visible: bool,
    pub message: String,
    pub toast_type: ToastType,
    pub show_until: Option<Instant>,
}

/// Type of toast notification
#[derive(Clone, Debug, PartialEq)]
pub enum ToastType {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastState {
    /// Create a new hidden toast
    pub fn new() -> Self {
        Self {
            visible: false,
            message: String::new(),
            toast_type: ToastType::Info,
            show_until: None,
        }
    }

    /// Show a toast with specified message and type for given duration
    pub fn show(&mut self, message: String, toast_type: ToastType, duration: Duration) {
        self.visible = true;
        self.message = message;
        self.toast_type = toast_type;
        self.show_until = Some(Instant::now() + duration);
    }

    /// Update toast state - hide if expired
    pub fn update(&mut self) {
        if let Some(until) = self.show_until {
            if Instant::now() >= until {
                self.hide();
            }
        }
    }

    /// Hide the toast
    pub fn hide(&mut self) {
        self.visible = false;
        self.show_until = None;
    }
}

/// Simple TUI application with complete internal state for drawing
pub struct TuiApp {
    // Terminal management
    terminal: Terminal<CrosstermBackend<Stdout>>,
    should_quit: bool,
    
    // 1. Input string state
    pub search_input: String,
    
    // 2. Search results array
    pub search_results: Vec<String>,
    
    // 3. Result cursor position information  
    pub selected_result_index: Option<usize>,
    
    // 4. Toast state (display/hide and content)
    pub toast_state: ToastState,
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
            should_quit: false,
            search_input: String::new(),
            search_results: Vec::new(),
            selected_result_index: None,
            toast_state: ToastState::new(),
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

            // Result navigation
            KeyCode::Down | KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_cursor_down();
            }
            KeyCode::Up | KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_cursor_up();
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

            // Enter to trigger search or select result
            KeyCode::Enter => {
                if self.selected_result_index.is_some() {
                    self.handle_result_selection();
                } else {
                    self.update_search_results();
                }
            }

            _ => {}
        }
    }

    /// Update search results based on current input
    fn update_search_results(&mut self) {
        self.search_results.clear();
        self.selected_result_index = None;
        
        if !self.search_input.is_empty() {
            // Add some mock results for demonstration
            for i in 1..=5 {
                self.search_results.push(format!(
                    "src/file_{}.rs:{}:Mock result for '{}'",
                    i, i * 10, self.search_input
                ));
            }
            
            // Set cursor to first result if we have results
            if !self.search_results.is_empty() {
                self.selected_result_index = Some(0);
            }
            
            // Show toast with search info
            self.toast_state.show(
                format!("Found {} results for '{}'", self.search_results.len(), self.search_input),
                ToastType::Info,
                Duration::from_secs(2),
            );
        }
    }

    /// Move cursor down to next result
    fn move_cursor_down(&mut self) {
        if !self.search_results.is_empty() {
            match self.selected_result_index {
                Some(index) => {
                    if index + 1 < self.search_results.len() {
                        self.selected_result_index = Some(index + 1);
                    } else {
                        // Wrap to first result
                        self.selected_result_index = Some(0);
                    }
                }
                None => {
                    self.selected_result_index = Some(0);
                }
            }
        }
    }

    /// Move cursor up to previous result
    fn move_cursor_up(&mut self) {
        if !self.search_results.is_empty() {
            match self.selected_result_index {
                Some(index) => {
                    if index > 0 {
                        self.selected_result_index = Some(index - 1);
                    } else {
                        // Wrap to last result
                        self.selected_result_index = Some(self.search_results.len() - 1);
                    }
                }
                None => {
                    self.selected_result_index = Some(self.search_results.len() - 1);
                }
            }
        }
    }

    /// Handle result selection (Enter key on selected result)
    fn handle_result_selection(&mut self) {
        if let Some(index) = self.selected_result_index {
            if let Some(result) = self.search_results.get(index) {
                // Show toast with selected result info
                self.toast_state.show(
                    format!("Selected: {}", result),
                    ToastType::Success,
                    Duration::from_secs(3),
                );
            }
        }
    }

    /// Draw the UI
    fn draw(&mut self) -> Result<()> {
        // Update toast state
        self.toast_state.update();
        
        let search_input = self.search_input.clone();
        let search_results = self.search_results.clone();
        let selected_index = self.selected_result_index;
        let toast_state = self.toast_state.clone();
        
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
            render_results_box(f, chunks[1], &search_results, selected_index);

            // 3. Status bar
            render_status_bar(f, chunks[2]);
            
            // 4. Toast (if visible)
            if toast_state.visible {
                render_toast(f, &toast_state);
            }
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

/// Render the results box with cursor highlighting
fn render_results_box(f: &mut Frame, area: ratatui::layout::Rect, search_results: &[String], selected_index: Option<usize>) {
    let items: Vec<ListItem> = search_results
        .iter()
        .enumerate()
        .map(|(i, result)| {
            let item = ListItem::new(result.as_str());
            if Some(i) == selected_index {
                item.style(Style::default().fg(Color::Black).bg(Color::White))
            } else {
                item
            }
        })
        .collect();

    let title = if let Some(index) = selected_index {
        format!("Search Results ({}/{})", index + 1, search_results.len())
    } else {
        "Search Results".to_string()
    };

    let results_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(Color::White));

    f.render_widget(results_list, area);
}

/// Render the status bar
fn render_status_bar(f: &mut Frame, area: ratatui::layout::Rect) {
    let help_text = "Type to search | ↑↓/Ctrl+P/N: Navigate | Enter: Select | Esc/Ctrl+C: Quit";
    let status = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .style(Style::default().fg(Color::Gray));
    f.render_widget(status, area);
}

/// Render toast notification
fn render_toast(f: &mut Frame, toast_state: &ToastState) {
    use ratatui::{
        layout::Alignment,
        widgets::Clear,
    };
    
    // Create a top-right positioned popup area (35% width, 15% height)
    let popup_area = top_right_rect(35, 15, f.size());
    
    // Clear the area first
    f.render_widget(Clear, popup_area);
    
    // Choose color and title based on toast type
    let (border_color, text_color, title) = match toast_state.toast_type {
        ToastType::Info => (Color::Blue, Color::White, "🔔 Info"),
        ToastType::Success => (Color::Green, Color::White, "✅ Success"),
        ToastType::Warning => (Color::Yellow, Color::Black, "⚠️ Warning"),
        ToastType::Error => (Color::Red, Color::White, "❌ Error"),
    };
    
    let toast_widget = Paragraph::new(toast_state.message.as_str())
        .block(Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color)))
        .style(Style::default().fg(text_color))
        .alignment(Alignment::Left)
        .wrap(ratatui::widgets::Wrap { trim: true });
    
    f.render_widget(toast_widget, popup_area);
}

/// Helper function to create top-right positioned rectangle
fn top_right_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    use ratatui::layout::{Constraint, Direction, Layout};
    
    // Create vertical layout: top area for toast, rest for main content
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(percent_y),  // Top area for toast
            Constraint::Percentage(100 - percent_y), // Rest of the screen
        ])
        .split(r);

    // Create horizontal layout in the top area: left space, right area for toast
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(100 - percent_x), // Left space
            Constraint::Percentage(percent_x),       // Right area for toast
        ])
        .split(vertical_chunks[0])[1] // Take the right part of the top area
}

/// Helper function to create centered rectangle (kept for potential future use)
#[allow(dead_code)]
fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    use ratatui::layout::{Constraint, Direction, Layout};
    
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}