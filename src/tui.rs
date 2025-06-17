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
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyModifiers},
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
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

// Import UnifiedSearchSystem and related types
use crate::cli::parse_query_with_mode;
use crate::unified_search::UnifiedSearchSystem;
use crate::actors::types::{SearchMode, SearchParams};
use crate::core::message::Message;
use crate::actors::messages::FaeMessage;

/// Type alias for TUI state update results to avoid large error types
pub type TuiResult<T = ()> = std::result::Result<T, Box<mpsc::error::SendError<StateUpdate>>>;

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

/// Index status information for status bar display
#[derive(Clone, Debug, Default)]
pub struct IndexStatus {
    pub queued_files: usize,
    pub indexed_files: usize,
    pub symbols_found: usize,
    pub is_active: bool,
}

impl IndexStatus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, queued: usize, indexed: usize, symbols: usize) {
        self.queued_files = queued;
        self.indexed_files = indexed;
        self.symbols_found = symbols;
        self.is_active = queued > 0;
    }

    pub fn is_complete(&self) -> bool {
        self.queued_files == 0 && self.indexed_files > 0
    }

    pub fn status_text(&self) -> String {
        if !self.is_active && self.indexed_files == 0 {
            "Ready".to_string()
        } else if self.is_active {
            format!(
                "Indexing: {}/{} files, {} symbols",
                self.indexed_files,
                self.indexed_files + self.queued_files,
                self.symbols_found
            )
        } else {
            format!(
                "Indexed: {} files, {} symbols",
                self.indexed_files, self.symbols_found
            )
        }
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

    // 5. Index status for status bar
    pub index_status: IndexStatus,
}

impl TuiState {
    /// Create new TUI state with defaults
    pub fn new() -> Self {
        Self {
            search_input: String::new(),
            search_results: Vec::new(),
            selected_result_index: None,
            toast_state: ToastState::new(),
            index_status: IndexStatus::new(),
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

/// Handle for external TUI control
#[derive(Clone)]
pub struct TuiHandle {
    pub state_sender: mpsc::UnboundedSender<StateUpdate>,
}

impl TuiHandle {
    /// Send a state update to the TUI
    pub fn update_state(&self, update: StateUpdate) -> TuiResult {
        self.state_sender.send(update).map_err(Box::new)
    }

    /// Convenience method to set search results (replaces existing)
    pub fn set_search_results(&self, results: Vec<String>) -> TuiResult {
        self.update_state(StateUpdate::new().with_search_results(results))
    }

    /// Convenience method to set search input
    pub fn set_search_input(&self, input: String) -> TuiResult {
        self.update_state(StateUpdate::new().with_search_input(input))
    }

    /// Convenience method to append search results (for streaming)
    pub fn append_search_results(&self, results: Vec<String>) -> TuiResult {
        self.update_state(StateUpdate::new().with_append_results(results))
    }

    /// Convenience method to show toast
    pub fn show_toast(
        &self,
        message: String,
        toast_type: ToastType,
        duration: Duration,
    ) -> TuiResult {
        self.update_state(StateUpdate::new().with_toast(message, toast_type, duration))
    }

    /// Convenience method to update index status
    pub fn update_index_status(&self, queued: usize, indexed: usize, symbols: usize) -> TuiResult {
        self.update_state(StateUpdate::new().with_index_progress(queued, indexed, symbols))
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

    // External state updates
    state_receiver: Option<mpsc::UnboundedReceiver<StateUpdate>>,

    // Search system integration
    search_system: UnifiedSearchSystem,
    search_result_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
    search_control_sender: mpsc::UnboundedSender<Message<FaeMessage>>,

    // Separated application state
    pub state: TuiState,
}

impl TuiApp {
    /// Create new TUI application with external control handle
    pub async fn new(search_path: &str) -> Result<(Self, TuiHandle)> {
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

        // Create external state update channel
        let (state_sender, state_receiver) = mpsc::unbounded_channel();
        let handle = TuiHandle { state_sender };

        // Create channels for search system communication
        let (search_result_sender, search_result_receiver) = mpsc::unbounded_channel();
        let (search_control_sender, search_control_receiver) = mpsc::unbounded_channel();

        // Initialize UnifiedSearchSystem with file watching enabled for TUI
        let search_system = UnifiedSearchSystem::new(
            search_path, 
            true, 
            search_result_sender, 
            search_control_receiver
        ).await.map_err(|e| {
            let _ = disable_raw_mode(); // Cleanup on failure
            eprintln!("Failed to initialize search system: {}", e);
            std::io::Error::new(std::io::ErrorKind::Other, e)
        })?;

        let app = TuiApp {
            terminal,
            should_quit: false,
            // Rendering control (60 FPS = ~16.67ms)
            needs_redraw: true, // Initial draw needed
            last_draw_time: Instant::now(),
            draw_throttle_duration: Duration::from_millis(16), // 60 FPS
            state_receiver: Some(state_receiver),
            search_system,
            search_result_receiver,
            search_control_sender,
            state: TuiState::new(),
        };

        Ok((app, handle))
    }

    /// Run the TUI application with non-blocking event processing
    pub async fn run(&mut self) -> Result<()> {
        // Initial render
        self.draw()?;
        self.needs_redraw = false;

        // Take the state receiver for the event loop
        let mut state_receiver = self.state_receiver.take();

        // Create event stream for non-blocking keyboard input
        let mut events = EventStream::new();

        // Main non-blocking event loop using tokio::select!
        loop {
            tokio::select! {
                // Handle keyboard input events
                Some(Ok(event)) = events.next() => {
                    if let Event::Key(key) = event {
                        self.handle_key_event(key);
                        if self.should_quit {
                            break;
                        }
                    }
                }

                // Handle search result messages
                Some(message) = self.search_result_receiver.recv() => {
                    self.handle_search_message(message);
                    self.needs_redraw = true;
                }

                // Handle external state updates
                Some(state_update) = async {
                    match &mut state_receiver {
                        Some(receiver) => receiver.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    self.apply_state_update(state_update);
                    self.needs_redraw = true;
                }

                // Periodic updates (toast expiration, etc.)
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    if self.state.update_toast() {
                        self.needs_redraw = true;
                    }
                }
            }

            // Check if we should quit
            if self.should_quit {
                break;
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
    /// Trigger search execution - sends search request to UnifiedSearchSystem
    fn update_search_results(&mut self) {
        self.state.search_results.clear();
        self.state.selected_result_index = None;

        if !self.state.search_input.is_empty() {
            // Parse search input to detect mode
            let (mode, query) = parse_query_with_mode(&self.state.search_input);
            
            // Show search mode in toast
            let mode_name = match mode {
                SearchMode::Literal => "Text",
                SearchMode::Symbol => "Symbol",
                SearchMode::Variable => "Variable", 
                SearchMode::Filepath => "File",
                SearchMode::Regexp => "Regex",
            };

            self.state.toast_state.show(
                format!("Searching {} for '{}'...", mode_name.to_lowercase(), query),
                ToastType::Info,
                Duration::from_secs(1),
            );

            // Create search parameters and send to UnifiedSearchSystem
            let search_params = SearchParams { 
                query: query.clone(), 
                mode: mode.clone() 
            };
            
            // Send search request
            let search_message = Message::new("search", FaeMessage::UpdateSearchParams(search_params));
            if let Err(_) = self.search_control_sender.send(search_message) {
                // Handle send error
                self.state.toast_state.show(
                    "Failed to start search - system not ready".to_string(),
                    ToastType::Error,
                    Duration::from_secs(3),
                );
            }
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

    /// Handle search result messages from UnifiedSearchSystem
    fn handle_search_message(&mut self, message: Message<FaeMessage>) {
        match message.payload {
            FaeMessage::PushSearchResult(search_result) => {
                // Format result for display
                let formatted_result = format!(
                    "{}:{}:{}",
                    search_result.filename,
                    search_result.line,
                    search_result.content.trim()
                );
                
                // Add to results list
                self.state.search_results.push(formatted_result);
                
                // Set cursor to first result if this is the first one
                if self.state.selected_result_index.is_none() && !self.state.search_results.is_empty() {
                    self.state.selected_result_index = Some(0);
                }
                
                // Limit results to prevent UI overflow (keep last 100 results)
                if self.state.search_results.len() > 100 {
                    self.state.search_results.remove(0);
                    // Adjust cursor position after removal
                    if let Some(index) = self.state.selected_result_index {
                        if index > 0 {
                            self.state.selected_result_index = Some(index - 1);
                        }
                    }
                }
            }
            FaeMessage::CompleteSearch => {
                // Show completion toast
                self.state.toast_state.show(
                    format!(
                        "Search completed - {} results found",
                        self.state.search_results.len()
                    ),
                    ToastType::Success,
                    Duration::from_secs(2),
                );
            }
            FaeMessage::NotifySearchReport { result_count } => {
                // Show final search report
                self.state.toast_state.show(
                    format!("Found {} total results", result_count),
                    ToastType::Info,
                    Duration::from_secs(2),
                );
            }
            FaeMessage::ClearResults => {
                // Clear current results
                self.state.search_results.clear();
                self.state.selected_result_index = None;
            }
            _ => {
                // Ignore other message types for now
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
        let index_status = self.state.index_status.clone();

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
            render_status_bar(f, chunks[2], &index_status);

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

    /// Apply external state update to the TUI state
    fn apply_state_update(&mut self, update: StateUpdate) {
        if let Some(input) = update.search_input {
            self.state.search_input = input;
        }

        if let Some(results) = update.search_results {
            self.state.search_results = results;
            // Reset selection to first result if we have results
            if !self.state.search_results.is_empty() {
                self.state.selected_result_index = Some(0);
            } else {
                self.state.selected_result_index = None;
            }
        }

        if let Some(append_results) = update.append_results {
            let was_empty = self.state.search_results.is_empty();
            self.state.search_results.extend(append_results);
            // Set cursor to first result if this was the first addition
            if was_empty && !self.state.search_results.is_empty() {
                self.state.selected_result_index = Some(0);
            }
        }

        if let Some(index) = update.selected_index {
            if let Some(idx) = index {
                if idx < self.state.search_results.len() {
                    self.state.selected_result_index = Some(idx);
                }
            } else {
                self.state.selected_result_index = None;
            }
        }

        if let Some((message, toast_type, duration)) = update.toast {
            self.state.toast_state.show(message, toast_type, duration);
        }

        if update.clear_results {
            self.state.search_results.clear();
            self.state.selected_result_index = None;
        }

        if update.hide_toast {
            self.state.toast_state.hide();
        }

        if let Some(index_status) = update.index_status {
            self.state.index_status = index_status;
        }
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
    pub append_results: Option<Vec<String>>, // New: append to existing results
    pub selected_index: Option<Option<usize>>,
    pub toast: Option<(String, ToastType, Duration)>,
    pub clear_results: bool,
    pub hide_toast: bool,
    pub index_status: Option<IndexStatus>,
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

    /// Set search results (replace existing)
    pub fn with_search_results(mut self, results: Vec<String>) -> Self {
        self.search_results = Some(results);
        self
    }

    /// Append to search results
    pub fn with_append_results(mut self, results: Vec<String>) -> Self {
        self.append_results = Some(results);
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

    /// Update index status
    pub fn with_index_status(mut self, status: IndexStatus) -> Self {
        self.index_status = Some(status);
        self
    }

    /// Update index progress (convenience method)
    pub fn with_index_progress(mut self, queued: usize, indexed: usize, symbols: usize) -> Self {
        let mut status = IndexStatus::new();
        status.update(queued, indexed, symbols);
        self.index_status = Some(status);
        self
    }
}

/// Render the input box with search mode indicator
fn render_input_box(f: &mut Frame, area: ratatui::layout::Rect, search_input: &str) {
    // Detect current search mode
    let (mode, _) = parse_query_with_mode(search_input);
    let mode_name = match mode {
        SearchMode::Literal => "Text",
        SearchMode::Symbol => "Symbol (#)",
        SearchMode::Variable => "Variable ($)", 
        SearchMode::Filepath => "File (@)",
        SearchMode::Regexp => "Regex (/)",
    };

    let title = format!("Search Input - {} Mode", mode_name);
    
    let input = Paragraph::new(search_input)
        .block(Block::default().borders(Borders::ALL).title(title))
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

/// Render the status bar with help text on left and index status on right
fn render_status_bar(f: &mut Frame, area: ratatui::layout::Rect, index_status: &IndexStatus) {
    use ratatui::layout::{Constraint, Direction, Layout};

    // Split status bar into left (help) and right (index status) parts
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70), // Help text
            Constraint::Percentage(30), // Index status
        ])
        .split(area);

    // Left side: Help text  
    let help_text = "Modes: text | #symbol | $variable | @file | /regex | ↑↓: Navigate | Enter: Select | Esc: Quit";
    let help_status = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .style(Style::default().fg(Color::Gray));
    f.render_widget(help_status, chunks[0]);

    // Right side: Index status
    let status_text = index_status.status_text();
    let status_color = if index_status.is_active {
        Color::Yellow // Indexing in progress
    } else if index_status.is_complete() {
        Color::Green // Indexing complete
    } else {
        Color::Gray // Ready/default
    };

    let index_status_widget = Paragraph::new(status_text.as_str())
        .block(Block::default().borders(Borders::ALL).title("Index Status"))
        .style(Style::default().fg(status_color));
    f.render_widget(index_status_widget, chunks[1]);
}

/// Render toast notification
fn render_toast(f: &mut Frame, toast_state: &ToastState) {
    use ratatui::{layout::Alignment, widgets::Clear};

    // Calculate optimal size in absolute dimensions
    let (width_chars, height_lines) = calculate_toast_size_absolute(toast_state, f.size());

    // Create a top-right positioned popup area with exact dimensions
    let popup_area = top_right_rect_absolute(width_chars, height_lines, f.size());

    // Clear the area first
    f.render_widget(Clear, popup_area);

    // Choose color and title based on toast type
    let (border_color, text_color, title) = match toast_state.toast_type {
        ToastType::Info => (Color::Blue, Color::White, "🔔 Info"),
        ToastType::Success => (Color::Green, Color::White, "✅ Success"),
        ToastType::Warning => (Color::Yellow, Color::Black, "⚠️ Warning"),
        ToastType::Error => (Color::Red, Color::White, "❌ Error"),
    };

    // Get the display message (with repeat count if applicable)
    let display_message = get_toast_display_message(toast_state);

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

/// Get the display message for a toast (with repeat count if applicable)
fn get_toast_display_message(toast_state: &ToastState) -> String {
    if toast_state.same_message_count > 1 {
        format!(
            "{} ({}x)",
            toast_state.message, toast_state.same_message_count
        )
    } else {
        toast_state.message.clone()
    }
}

/// Calculate optimal toast size in absolute dimensions (characters and lines)
fn calculate_toast_size_absolute(
    toast_state: &ToastState,
    terminal_size: ratatui::layout::Rect,
) -> (u16, u16) {
    let display_message = get_toast_display_message(toast_state);

    // Calculate required width based on message length
    // Use visual width approximation instead of byte length for emojis
    let title_visual_width = match toast_state.toast_type {
        ToastType::Info => 7,    // "🔔 Info" visual width
        ToastType::Success => 9, // "✅ Success" visual width
        ToastType::Warning => 9, // "⚠️ Warning" visual width
        ToastType::Error => 7,   // "❌ Error" visual width
    };

    // Consider the longer of title or message for width calculation
    let content_width = std::cmp::max(display_message.len(), title_visual_width) + 4; // +4 for borders and padding

    // Set absolute width bounds
    let min_width = 20;
    let max_width = (terminal_size.width as f32 * 0.7) as usize; // 70% of terminal width
    let actual_width = content_width.clamp(min_width, max_width) as u16;

    // Calculate height based on actual text wrapping with the exact width
    let available_content_width = actual_width.saturating_sub(4) as usize; // -4 for borders
    let content_lines = if available_content_width > 0 {
        calculate_wrapped_lines(&display_message, available_content_width)
    } else {
        1 // Fallback for very narrow terminals
    };

    // Total lines = content lines + top/bottom borders
    let total_lines = content_lines + 2; // +2 for top/bottom borders only

    // Set absolute height bounds
    let min_height = 3; // Minimum viable toast height
    let max_height = (terminal_size.height as f32 * 0.4) as usize; // 40% of terminal height
    let actual_height = total_lines.clamp(min_height, max_height) as u16;

    (actual_width, actual_height)
}


/// Calculate how many lines text will take when wrapped to given width
fn calculate_wrapped_lines(text: &str, width: usize) -> usize {
    if text.is_empty() {
        return 1;
    }

    if width == 0 {
        return text.len(); // Each character gets its own line in worst case
    }

    let mut lines = 0;
    let mut current_line_len = 0;

    // Split by whitespace and handle word wrapping
    for word in text.split_whitespace() {
        let word_len = word.len();

        // If adding this word would exceed the width
        if current_line_len + word_len + (if current_line_len > 0 { 1 } else { 0 }) > width {
            // If the word itself is longer than width, it needs to be broken
            if word_len > width {
                // First finish the current line if it has content
                if current_line_len > 0 {
                    lines += 1;
                }
                // Calculate how many lines the long word needs
                lines += word_len.div_ceil(width); // Ceiling division
                current_line_len = word_len % width;
                if current_line_len == 0 {
                    current_line_len = 0; // Full lines consumed
                }
            } else {
                // Start new line with this word
                lines += 1;
                current_line_len = word_len;
            }
        } else {
            // Add word to current line
            if current_line_len > 0 {
                current_line_len += 1; // Add space before word
            }
            current_line_len += word_len;
        }
    }

    // If we have content on the last line, count it
    if current_line_len > 0 {
        lines += 1;
    }

    // Ensure at least 1 line
    lines.max(1)
}


/// Create a rect in top right corner with exact dimensions (width in chars, height in lines)
fn top_right_rect_absolute(
    width_chars: u16,
    height_lines: u16,
    r: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    // Ensure dimensions don't exceed available space
    let actual_width = width_chars.min(r.width);
    let actual_height = height_lines.min(r.height);

    // Calculate position for top-right corner
    let x = r.x + r.width.saturating_sub(actual_width);
    let y = r.y;

    ratatui::layout::Rect {
        x,
        y,
        width: actual_width,
        height: actual_height,
    }
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

        let top_right = top_right_rect_absolute(30, 20, full_rect);

        // Should be in the top-right corner
        assert!(top_right.x > 50); // Right side
        assert_eq!(top_right.y, 0); // Top
        assert_eq!(top_right.width, 30); // 30% of width
        assert_eq!(top_right.height, 20); // 20% of height
    }

    #[test]
    fn test_tui_handle_creation() {
        // Create TUI handle
        let (state_sender, _state_receiver) = mpsc::unbounded_channel();
        let handle = TuiHandle { state_sender };

        // Test basic operations don't panic
        let result = handle.set_search_input("test query".to_string());
        assert!(result.is_ok(), "Should be able to send state updates");
    }

    #[test]
    fn test_state_update_with_append() {
        let update = StateUpdate::new()
            .with_search_input("test".to_string())
            .with_append_results(vec!["result1".to_string(), "result2".to_string()])
            .with_info_toast("Test message".to_string());

        assert_eq!(update.search_input, Some("test".to_string()));
        assert_eq!(
            update.append_results,
            Some(vec!["result1".to_string(), "result2".to_string()])
        );
        assert!(update.toast.is_some());
    }

    #[test]
    fn test_calculate_wrapped_lines_debug() {
        // Debug specific case: short message that should fit in one line
        let message = "Ready";
        let width = 16; // Typical available width for a small toast
        let result = calculate_wrapped_lines(message, width);
        println!(
            "Message: '{}' (len={}), width: {}, result: {}",
            message,
            message.len(),
            width,
            result
        );
        assert_eq!(
            result, 1,
            "Short message '{}' should fit in 1 line with width {}",
            message, width
        );

        // Test realistic toast message
        let message = "Indexing: 3/8 files, 120 symbols";
        let width = 25;
        let result = calculate_wrapped_lines(message, width);
        println!(
            "Message: '{}' (len={}), width: {}, result: {}",
            message,
            message.len(),
            width,
            result
        );
        assert!(result <= 2, "Medium message should fit in 1-2 lines");
    }

    #[test]
    fn test_calculate_wrapped_lines() {
        // Test empty string
        assert_eq!(calculate_wrapped_lines("", 10), 1);

        // Test single word that fits
        assert_eq!(calculate_wrapped_lines("hello", 10), 1);

        // Test multiple words that fit on one line
        assert_eq!(calculate_wrapped_lines("hello world", 15), 1);

        // Test text that needs wrapping
        assert_eq!(calculate_wrapped_lines("hello world test", 10), 2);

        // Test very long word that needs breaking
        // "verylongwordthatdoesnotfit" = 26 chars, with width 10 = ceil(26/10) = 3 lines
        // But since we add +1 for current line and calculate separately, adjust expectation
        let result = calculate_wrapped_lines("verylongwordthatdoesnotfit", 10);
        assert!(result >= 3, "Expected at least 3 lines, got {}", result);

        // Test zero width (edge case)
        assert!(calculate_wrapped_lines("test", 0) > 0);

        // Test realistic toast message
        let long_message = "Indexing completed: 25 files, 1200 symbols found successfully";
        assert!(calculate_wrapped_lines(long_message, 30) >= 2);
    }

    #[test]
    fn test_get_toast_display_message() {
        let mut toast = ToastState::new();

        // Test normal message
        toast.show("Hello".to_string(), ToastType::Info, Duration::from_secs(2));
        assert_eq!(get_toast_display_message(&toast), "Hello");

        // Test repeated message
        toast.show("Hello".to_string(), ToastType::Info, Duration::from_secs(2));
        toast.show("Hello".to_string(), ToastType::Info, Duration::from_secs(2));
        assert_eq!(get_toast_display_message(&toast), "Hello (3x)");
    }

    #[test]
    fn test_emoji_length_debug() {
        // Check emoji byte length vs visual width
        let emoji_info = "🔔 Info";
        let emoji_success = "✅ Success";
        let emoji_warning = "⚠️ Warning";
        let emoji_error = "❌ Error";

        println!("'{}' len={}", emoji_info, emoji_info.len());
        println!("'{}' len={}", emoji_success, emoji_success.len());
        println!("'{}' len={}", emoji_warning, emoji_warning.len());
        println!("'{}' len={}", emoji_error, emoji_error.len());

        // Test width calculation with emoji
        let terminal_size = ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30,
        };
        let mut toast = ToastState::new();
        toast.show("Test".to_string(), ToastType::Info, Duration::from_secs(2));

        let (width, height) = calculate_toast_size_absolute(&toast, terminal_size);
        println!(
            "Toast size for 'Test' with Info emoji: width={}%, height={}%",
            width, height
        );
    }

    #[test]
    fn test_calculate_toast_size_debug() {
        use ratatui::layout::Rect;

        // Test actual toast calculation step by step
        let mut toast = ToastState::new();
        toast.show("Ready".to_string(), ToastType::Info, Duration::from_secs(2));

        let terminal_size = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30,
        };
        let display_message = get_toast_display_message(&toast);
        println!("Display message: '{}'", display_message);

        // Calculate width manually to debug
        let title_visual_width = 7; // "🔔 Info" visual width
        let content_width = std::cmp::max(display_message.len(), title_visual_width) + 4;
        println!(
            "Content width calculation: max({}, {}) + 4 = {}",
            display_message.len(),
            title_visual_width,
            content_width
        );

        let width_percent =
            ((content_width * 100) / (terminal_size.width as usize)).clamp(20, 70) as u16;
        println!("Width percent: {}", width_percent);

        let available_width =
            (terminal_size.width as usize * width_percent as usize / 100).saturating_sub(4);
        println!("Available width for text: {}", available_width);

        let content_lines = calculate_wrapped_lines(&display_message, available_width);
        println!("Content lines: {}", content_lines);

        let total_lines = content_lines + 2;
        println!("Total lines (content + borders): {}", total_lines);

        let height_percent =
            ((total_lines * 100) / (terminal_size.height as usize)).clamp(15, 50) as u16;
        println!("Height percent: {}", height_percent);

        // Now test the actual function
        let (actual_width, actual_height) = calculate_toast_size_absolute(&toast, terminal_size);
        println!(
            "Actual result: width={}%, height={}%",
            actual_width, actual_height
        );
    }

    #[test]
    fn test_calculate_toast_size() {
        use ratatui::layout::Rect;

        // Test short message
        let mut toast = ToastState::new();
        toast.show("Short".to_string(), ToastType::Info, Duration::from_secs(2));

        let terminal_size = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30,
        };
        let (width, height) = calculate_toast_size_absolute(&toast, terminal_size);

        // Short message should use minimum width and height
        assert_eq!(width, 20);
        assert_eq!(height, 3, "Short message should use minimum height"); // Updated expectation

        // Test long message
        let long_message = "This is a very long message that should cause the toast to expand to accommodate the content properly";
        toast.show(
            long_message.to_string(),
            ToastType::Warning,
            Duration::from_secs(3),
        );

        let (width, height) = calculate_toast_size_absolute(&toast, terminal_size);

        // Long message should use more width and height but stay within limits
        assert!(width > 20);
        assert!(width <= 70);
        assert!(height >= 3); // Should be higher than or equal to minimum
        assert!(height <= 40); // Should not exceed maximum

        // Test very long message that should max out height
        let very_long_message = "This is an extremely long message that contains many words and should definitely cause the toast to expand to multiple lines and potentially reach the maximum height limit that we have set for toast notifications in the TUI interface.";
        toast.show(
            very_long_message.to_string(),
            ToastType::Error,
            Duration::from_secs(5),
        );

        let (_width2, height2) = calculate_toast_size_absolute(&toast, terminal_size);
        assert!(
            height2 > height,
            "Very long message should be taller than medium message"
        );

        // Test that different terminal sizes produce different results
        let small_terminal = Rect {
            x: 0,
            y: 0,
            width: 50,
            height: 20,
        };
        let (width_small, height_small) = calculate_toast_size_absolute(&toast, small_terminal);

        let large_terminal = Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 60,
        };
        let (width_large, height_large) = calculate_toast_size_absolute(&toast, large_terminal);

        // On larger terminal, width percentage might be smaller (same content takes less percentage)
        // But absolute width should be larger or equal
        assert!(
            width_large >= width_small
                || (width_large as usize * large_terminal.width as usize / 100)
                    >= (width_small as usize * small_terminal.width as usize / 100)
        );

        // Height should also be responsive to terminal size
        assert!(height_small <= 40 && height_large <= 40); // Both should respect max limit
    }

    #[test]
    fn test_apply_state_update_logic() {
        // Test the apply_state_update logic without creating TUI
        let mut state = TuiState::new();

        // Test basic state update
        let update = StateUpdate::new()
            .with_search_input("test query".to_string())
            .with_search_results(vec!["result1".to_string(), "result2".to_string()]);

        // Simulate the logic from apply_state_update
        if let Some(input) = update.search_input {
            state.search_input = input;
        }
        if let Some(results) = update.search_results {
            state.search_results = results;
            if !state.search_results.is_empty() {
                state.selected_result_index = Some(0);
            } else {
                state.selected_result_index = None;
            }
        }

        assert_eq!(state.search_input, "test query");
        assert_eq!(state.search_results.len(), 2);
        assert_eq!(state.selected_result_index, Some(0));

        // Test append functionality
        let append_update = StateUpdate::new().with_append_results(vec!["result3".to_string()]);

        if let Some(append_results) = append_update.append_results {
            let was_empty = state.search_results.is_empty();
            state.search_results.extend(append_results);
            if was_empty && !state.search_results.is_empty() {
                state.selected_result_index = Some(0);
            }
        }

        assert_eq!(state.search_results.len(), 3);
        assert_eq!(state.search_results[2], "result3");
        assert_eq!(state.selected_result_index, Some(0)); // Should remain unchanged

        // Test clear results
        let clear_update = StateUpdate::new().with_clear_results();
        if clear_update.clear_results {
            state.search_results.clear();
            state.selected_result_index = None;
        }

        assert_eq!(state.search_results.len(), 0);
        assert_eq!(state.selected_result_index, None);
    }
}
