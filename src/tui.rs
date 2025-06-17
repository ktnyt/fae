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
//!
//! ## External State Updates
//!
//! The TUI state can be updated from external sources:
//!
//! ```rust,no_run
//! # use fae::tui::{TuiApp, StateUpdate, ToastType};
//! # use std::time::Duration;
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut app = TuiApp::new(".").await?;
//!
//! // Individual updates
//! app.set_search_input("test query".to_string());
//! app.set_search_results(vec!["result1".to_string(), "result2".to_string()]);
//! app.show_toast("Search completed".to_string(), ToastType::Success, Duration::from_secs(3));
//!
//! // Batch updates
//! app.update_state_batch(
//!     StateUpdate::new()
//!         .with_search_input("new query".to_string())
//!         .with_search_results(vec!["result1".to_string()])
//!         .with_success_toast("Found results!".to_string())
//! )?;
//! # Ok(())
//! # }
//! ```

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
    // Auto-close tracking
    last_message: String,
    last_change_time: Instant,
    pub same_message_count: u32,
}

/// Type of toast notification
#[derive(Clone, Debug, PartialEq)]
pub enum ToastType {
    Info,
    Success,
    Warning,
    Error,
}

impl Default for ToastState {
    fn default() -> Self {
        Self::new()
    }
}

impl ToastState {
    /// Create a new hidden toast
    pub fn new() -> Self {
        Self {
            visible: false,
            message: String::new(),
            toast_type: ToastType::Info,
            show_until: None,
            last_message: String::new(),
            last_change_time: Instant::now(),
            same_message_count: 0,
        }
    }

    /// Show a toast with specified message and type for given duration
    pub fn show(&mut self, message: String, toast_type: ToastType, duration: Duration) {
        // Check if this is the same message as before
        if self.last_message == message {
            self.same_message_count += 1;
            // If same message appears 3+ times, reduce display duration
            let adjusted_duration = if self.same_message_count >= 3 {
                Duration::from_millis(500) // Very short duration for repeated messages
            } else {
                duration
            };
            self.show_until = Some(Instant::now() + adjusted_duration);
        } else {
            // New message - reset tracking
            self.same_message_count = 1;
            self.last_message = message.clone();
            self.last_change_time = Instant::now();
            self.show_until = Some(Instant::now() + duration);
        }

        self.visible = true;
        self.message = message;
        self.toast_type = toast_type;
    }

    /// Update toast state - hide if expired or stale
    pub fn update(&mut self) {
        if let Some(until) = self.show_until {
            if Instant::now() >= until {
                self.hide();
                return;
            }
        }

        // Auto-close if same state for too long (30 seconds without change)
        if self.visible && self.last_change_time.elapsed() > Duration::from_secs(30) {
            self.hide();
        }
    }

    /// Hide the toast
    pub fn hide(&mut self) {
        self.visible = false;
        self.show_until = None;
        // Reset tracking when hiding
        self.same_message_count = 0;
        self.last_change_time = Instant::now();
    }
}

/// TUI application state (separated from terminal management)
#[derive(Clone, Debug)]
pub struct TuiState {
    // 1. Input string state
    pub search_input: String,

    // 2. Search results array
    pub search_results: Vec<String>,

    // 3. Result cursor position information
    pub selected_result_index: Option<usize>,

    // 4. Toast state (display/hide and content)
    pub toast_state: ToastState,
}

impl TuiState {
    /// Create new TUI state with defaults
    pub fn new() -> Self {
        Self {
            search_input: String::new(),
            search_results: Vec::new(),
            selected_result_index: None,
            toast_state: ToastState::new(),
        }
    }

    /// Update toast state and return true if redraw is needed
    pub fn update_toast(&mut self) -> bool {
        let was_visible = self.toast_state.visible;
        self.toast_state.update();
        was_visible != self.toast_state.visible
    }

    /// Check if any state requires periodic updates
    pub fn needs_periodic_update(&self) -> bool {
        self.toast_state.visible || self.toast_state.show_until.is_some()
    }
}

impl Default for TuiState {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple TUI application with separated state management
pub struct TuiApp {
    // Terminal management
    terminal: Terminal<CrosstermBackend<Stdout>>,
    should_quit: bool,

    // Rendering control
    needs_redraw: bool,
    last_draw_time: Instant,
    draw_throttle_duration: Duration,

    // Separated application state
    pub state: TuiState,
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
            // Rendering control (60 FPS = ~16.67ms)
            needs_redraw: true, // Initial draw needed
            last_draw_time: Instant::now(),
            draw_throttle_duration: Duration::from_millis(16), // 60 FPS
            state: TuiState::new(),
        })
    }

    /// Run the TUI application
    pub async fn run(&mut self) -> Result<()> {
        // Initial render
        self.draw()?;
        self.needs_redraw = false;

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

            // 🔧 Fix: Update toast state periodically to ensure auto-close works
            if self.state.update_toast() {
                self.needs_redraw = true;
            }

            // Only redraw if needed and enough time has passed (throttling)
            if self.needs_redraw {
                let now = Instant::now();
                if now.duration_since(self.last_draw_time) >= self.draw_throttle_duration {
                    self.draw()?;
                    self.needs_redraw = false;
                    self.last_draw_time = now;
                }
            }

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
                self.needs_redraw = true;
            }
            KeyCode::Up | KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_cursor_up();
                self.needs_redraw = true;
            }

            // Text input
            KeyCode::Char(c) => {
                self.state.search_input.push(c);
                self.update_search_results();
                self.needs_redraw = true;
            }
            KeyCode::Backspace => {
                self.state.search_input.pop();
                self.update_search_results();
                self.needs_redraw = true;
            }

            // Enter to trigger search or select result
            KeyCode::Enter => {
                if self.state.selected_result_index.is_some() {
                    self.handle_result_selection();
                } else {
                    self.update_search_results();
                }
                self.needs_redraw = true;
            }

            _ => {}
        }
    }

    /// Update search results based on current input
    fn update_search_results(&mut self) {
        self.state.search_results.clear();
        self.state.selected_result_index = None;

        if !self.state.search_input.is_empty() {
            // Add some mock results for demonstration
            for i in 1..=5 {
                self.state.search_results.push(format!(
                    "src/file_{}.rs:{}:Mock result for '{}'",
                    i,
                    i * 10,
                    self.state.search_input
                ));
            }

            // Set cursor to first result if we have results
            if !self.state.search_results.is_empty() {
                self.state.selected_result_index = Some(0);
            }

            // Show toast with search info
            self.state.toast_state.show(
                format!(
                    "Found {} results for '{}'",
                    self.state.search_results.len(),
                    self.state.search_input
                ),
                ToastType::Info,
                Duration::from_secs(2),
            );
        }
    }

    /// Move cursor down to next result
    fn move_cursor_down(&mut self) {
        if !self.state.search_results.is_empty() {
            match self.state.selected_result_index {
                Some(index) => {
                    if index + 1 < self.state.search_results.len() {
                        self.state.selected_result_index = Some(index + 1);
                    } else {
                        // Wrap to first result
                        self.state.selected_result_index = Some(0);
                    }
                }
                None => {
                    self.state.selected_result_index = Some(0);
                }
            }
        }
    }

    /// Move cursor up to previous result
    fn move_cursor_up(&mut self) {
        if !self.state.search_results.is_empty() {
            match self.state.selected_result_index {
                Some(index) => {
                    if index > 0 {
                        self.state.selected_result_index = Some(index - 1);
                    } else {
                        // Wrap to last result
                        self.state.selected_result_index =
                            Some(self.state.search_results.len() - 1);
                    }
                }
                None => {
                    self.state.selected_result_index = Some(self.state.search_results.len() - 1);
                }
            }
        }
    }

    /// Handle result selection (Enter key on selected result)
    fn handle_result_selection(&mut self) {
        if let Some(index) = self.state.selected_result_index {
            if let Some(result) = self.state.search_results.get(index) {
                // Show toast with selected result info
                self.state.toast_state.show(
                    format!("Selected: {}", result),
                    ToastType::Success,
                    Duration::from_secs(3),
                );
            }
        }
    }

    /// Draw the UI
    fn draw(&mut self) -> Result<()> {
        // 🔧 Fix: Remove duplicate toast update logic (now handled in run() method)
        // Toast state is already updated in the main event loop

        let search_input = self.state.search_input.clone();
        let search_results = self.state.search_results.clone();
        let selected_index = self.state.selected_result_index;
        let toast_state = self.state.toast_state.clone();

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

    // ===== External State Update API =====

    /// Update search input from external source
    pub fn set_search_input(&mut self, input: String) {
        self.state.search_input = input;
        self.needs_redraw = true;
    }

    /// Add a search result from external source
    pub fn add_search_result(&mut self, result: String) {
        self.state.search_results.push(result);
        // Auto-select first result if none selected
        if self.state.selected_result_index.is_none() && !self.state.search_results.is_empty() {
            self.state.selected_result_index = Some(0);
        }
        self.needs_redraw = true;
    }

    /// Set all search results from external source
    pub fn set_search_results(&mut self, results: Vec<String>) {
        self.state.search_results = results;
        // Reset selection to first result if we have results
        if !self.state.search_results.is_empty() {
            self.state.selected_result_index = Some(0);
        } else {
            self.state.selected_result_index = None;
        }
        self.needs_redraw = true;
    }

    /// Clear all search results
    pub fn clear_search_results(&mut self) {
        self.state.search_results.clear();
        self.state.selected_result_index = None;
        self.needs_redraw = true;
    }

    /// Set cursor position from external source
    pub fn set_selected_result_index(&mut self, index: Option<usize>) {
        if let Some(idx) = index {
            if idx < self.state.search_results.len() {
                self.state.selected_result_index = Some(idx);
            }
        } else {
            self.state.selected_result_index = None;
        }
        self.needs_redraw = true;
    }

    /// Show toast notification from external source
    pub fn show_toast(&mut self, message: String, toast_type: ToastType, duration: Duration) {
        self.state.toast_state.show(message, toast_type, duration);
        self.needs_redraw = true;
    }

    /// Hide current toast
    pub fn hide_toast(&mut self) {
        self.state.toast_state.hide();
        self.needs_redraw = true;
    }

    // ===== External State Access API =====

    /// Get current search input
    pub fn get_search_input(&self) -> &str {
        &self.state.search_input
    }

    /// Get current search results
    pub fn get_search_results(&self) -> &[String] {
        &self.state.search_results
    }

    /// Get currently selected result index
    pub fn get_selected_result_index(&self) -> Option<usize> {
        self.state.selected_result_index
    }

    /// Get currently selected result
    pub fn get_selected_result(&self) -> Option<&str> {
        if let Some(index) = self.state.selected_result_index {
            self.state.search_results.get(index).map(|s| s.as_str())
        } else {
            None
        }
    }

    /// Check if toast is currently visible
    pub fn is_toast_visible(&self) -> bool {
        self.state.toast_state.visible
    }

    /// Force a UI redraw (useful after external state updates)
    pub fn force_redraw(&mut self) -> Result<()> {
        self.needs_redraw = true;
        self.draw()
    }

    // ===== Batch Update API =====

    /// Update multiple state elements at once and redraw
    pub fn update_state_batch(&mut self, updates: StateUpdate) -> Result<()> {
        if let Some(input) = updates.search_input {
            self.set_search_input(input);
        }

        if let Some(results) = updates.search_results {
            self.set_search_results(results);
        }

        if let Some(index) = updates.selected_index {
            self.set_selected_result_index(index);
        }

        if let Some((message, toast_type, duration)) = updates.toast {
            self.show_toast(message, toast_type, duration);
        }

        if updates.clear_results {
            self.clear_search_results();
        }

        if updates.hide_toast {
            self.hide_toast();
        }

        // Mark for redraw after batch update
        self.needs_redraw = true;
        self.draw()
    }
}

/// Batch state update structure for external integration
#[derive(Default, Debug)]
pub struct StateUpdate {
    pub search_input: Option<String>,
    pub search_results: Option<Vec<String>>,
    pub selected_index: Option<Option<usize>>,
    pub toast: Option<(String, ToastType, Duration)>,
    pub clear_results: bool,
    pub hide_toast: bool,
}

impl StateUpdate {
    /// Create a new empty state update
    pub fn new() -> Self {
        Self::default()
    }

    /// Set search input
    pub fn with_search_input(mut self, input: String) -> Self {
        self.search_input = Some(input);
        self
    }

    /// Set search results
    pub fn with_search_results(mut self, results: Vec<String>) -> Self {
        self.search_results = Some(results);
        self
    }

    /// Set selected index
    pub fn with_selected_index(mut self, index: Option<usize>) -> Self {
        self.selected_index = Some(index);
        self
    }

    /// Add toast notification
    pub fn with_toast(
        mut self,
        message: String,
        toast_type: ToastType,
        duration: Duration,
    ) -> Self {
        self.toast = Some((message, toast_type, duration));
        self
    }

    /// Add info toast (convenience method)
    pub fn with_info_toast(mut self, message: String) -> Self {
        self.toast = Some((message, ToastType::Info, Duration::from_secs(2)));
        self
    }

    /// Add success toast (convenience method)
    pub fn with_success_toast(mut self, message: String) -> Self {
        self.toast = Some((message, ToastType::Success, Duration::from_secs(3)));
        self
    }

    /// Clear search results
    pub fn with_clear_results(mut self) -> Self {
        self.clear_results = true;
        self
    }

    /// Hide toast
    pub fn with_hide_toast(mut self) -> Self {
        self.hide_toast = true;
        self
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
fn render_results_box(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    search_results: &[String],
    selected_index: Option<usize>,
) {
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
    use ratatui::{layout::Alignment, widgets::Clear};

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

    // Add repeat count indicator if message appeared multiple times
    let display_message = if toast_state.same_message_count > 1 {
        format!(
            "{} ({}x)",
            toast_state.message, toast_state.same_message_count
        )
    } else {
        toast_state.message.clone()
    };

    let toast_widget = Paragraph::new(display_message.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(border_color)),
        )
        .style(Style::default().fg(text_color))
        .alignment(Alignment::Left)
        .wrap(ratatui::widgets::Wrap { trim: true });

    f.render_widget(toast_widget, popup_area);
}

/// Helper function to create top-right positioned rectangle
fn top_right_rect(
    percent_x: u16,
    percent_y: u16,
    r: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    use ratatui::layout::{Constraint, Direction, Layout};

    // Create vertical layout: top area for toast, rest for main content
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(percent_y),       // Top area for toast
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
fn centered_rect(
    percent_x: u16,
    percent_y: u16,
    r: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_toast_state_creation() {
        let toast = ToastState::new();
        assert!(!toast.visible);
        assert_eq!(toast.same_message_count, 0);
    }

    #[test]
    fn test_toast_state_duplicate_messages() {
        let mut toast = ToastState::new();

        // First message
        toast.show("test".to_string(), ToastType::Info, Duration::from_secs(2));
        assert_eq!(toast.same_message_count, 1);

        // Same message again
        toast.show("test".to_string(), ToastType::Info, Duration::from_secs(2));
        assert_eq!(toast.same_message_count, 2);

        // Different message
        toast.show(
            "different".to_string(),
            ToastType::Info,
            Duration::from_secs(2),
        );
        assert_eq!(toast.same_message_count, 1);
    }

    #[test]
    fn test_state_update_builder() {
        let update = StateUpdate::new()
            .with_search_input("test query".to_string())
            .with_success_toast("Success!".to_string())
            .with_clear_results();

        assert_eq!(update.search_input, Some("test query".to_string()));
        assert!(update.clear_results);
        assert!(update.toast.is_some());

        if let Some((msg, toast_type, _)) = &update.toast {
            assert_eq!(msg, "Success!");
            assert_eq!(*toast_type, ToastType::Success);
        }
    }

    #[test]
    fn test_top_right_rect() {
        use ratatui::layout::Rect;

        let full_rect = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };

        let top_right = top_right_rect(30, 20, full_rect);

        // Should be in the top-right corner
        assert!(top_right.x > 50); // Right side
        assert_eq!(top_right.y, 0); // Top
        assert_eq!(top_right.width, 30); // 30% of width
        assert_eq!(top_right.height, 20); // 20% of height
    }
}
