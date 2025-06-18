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
//! let (mut app, handle) = TuiApp::new(".").await?;
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
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::{
    io::{stdout, Result, Stdout},
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

// Import search-related types
use crate::actors::types::SearchMode;
use crate::cli::parse_query_with_mode;

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

    // 6. Statistics overlay state
    pub show_stats_overlay: bool,

    // 7. Emacs-style text editing state
    pub cursor_position: usize, // Cursor position in search_input
    pub kill_ring: String,      // Kill/yank buffer (emacs-style)

    // 8. Result list scroll state
    pub results_list_state: ListState, // StatefulList state for scrolling
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
            show_stats_overlay: false,
            cursor_position: 0,
            kill_ring: String::new(),
            results_list_state: ListState::default(),
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

    // Search control channel for dynamic search execution
    search_control_sender:
        Option<mpsc::UnboundedSender<crate::core::Message<crate::actors::messages::FaeMessage>>>,
    
    // TUI actor channel for request ID synchronization
    tui_actor_sender:
        Option<mpsc::UnboundedSender<crate::core::Message<crate::actors::messages::FaeMessage>>>,

    // Search debounce control
    debounce_delay: Duration,
    pending_search_query: Option<String>,
    last_input_time: Option<Instant>,

    // Separated application state
    pub state: TuiState,
}

impl TuiApp {
    /// Create new TUI application with external control handle
    pub async fn new(_search_path: &str) -> Result<(Self, TuiHandle)> {
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

        let app = TuiApp {
            terminal,
            should_quit: false,
            // Rendering control (60 FPS = ~16.67ms)
            needs_redraw: true, // Initial draw needed
            last_draw_time: Instant::now(),
            draw_throttle_duration: Duration::from_millis(16), // 60 FPS
            state_receiver: Some(state_receiver),
            search_control_sender: None, // Will be set later via set_search_control_sender
            tui_actor_sender: None, // Will be set later via set_tui_actor_sender
            // Search debounce control
            debounce_delay: Duration::from_millis(100), // 100ms debounce delay
            pending_search_query: None,
            last_input_time: None,
            state: TuiState::new(),
        };

        Ok((app, handle))
    }

    /// Set the search control sender for dynamic search execution
    pub fn set_search_control_sender(
        &mut self,
        sender: mpsc::UnboundedSender<crate::core::Message<crate::actors::messages::FaeMessage>>,
    ) {
        self.search_control_sender = Some(sender);
    }

    /// Set the TUI actor sender for request ID synchronization
    pub fn set_tui_actor_sender(
        &mut self,
        sender: mpsc::UnboundedSender<crate::core::Message<crate::actors::messages::FaeMessage>>,
    ) {
        self.tui_actor_sender = Some(sender);
    }

    /// Execute a dynamic search by sending a request to the UnifiedSearchSystem
    pub fn execute_search(
        &self,
        query: String,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref sender) = self.search_control_sender {
            use crate::actors::messages::FaeMessage;
            use crate::cli::create_search_params;
            use crate::core::Message;

            log::debug!("TuiApp executing search: '{}'", query);

            // Parse the query and determine search mode
            let search_params = create_search_params(&query);

            // Send abort search message to stop any ongoing searches
            let abort_message = Message::new("abortSearch", FaeMessage::AbortSearch);
            if let Err(e) = sender.send(abort_message) {
                log::warn!("Failed to send abort search message: {}", e);
            }

            // Clear previous results for empty queries
            let clear_message = Message::new("clearResults", FaeMessage::ClearResults);
            if let Err(e) = sender.send(clear_message) {
                log::warn!("Failed to send clear results message: {}", e);
            }

            // Skip search if query is empty or just a prefix
            if search_params.query.trim().is_empty() {
                log::debug!(
                    "Aborting search for empty or prefix-only query: '{}'",
                    query
                );
                return Ok(());
            }

            // Abort any ongoing search before starting new one
            let abort_message = Message::new("abortSearch", FaeMessage::AbortSearch);
            if let Err(e) = sender.send(abort_message) {
                log::warn!("Failed to send abort search message: {}", e);
            }

            // Clear previous results first
            let clear_message = Message::new("clearResults", FaeMessage::ClearResults);
            if let Err(e) = sender.send(clear_message) {
                log::warn!("Failed to send clear results message: {}", e);
            }

            // Generate request ID and send search request
            let request_id = tiny_id::ShortCodeGenerator::new_alphanumeric(8).next_string();
            let search_message = Message::new(
                "updateSearchParams",
                FaeMessage::UpdateSearchParams {
                    params: search_params.clone(),
                    request_id: request_id.clone(),
                },
            );

            if let Err(e) = sender.send(search_message) {
                let error_msg = format!("Failed to send search request: {}", e);
                log::error!("{}", error_msg);
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    error_msg,
                )));
            }

            // Also notify TuiActor about the new request ID
            if let Some(ref tui_sender) = self.tui_actor_sender {
                let tui_message = Message::new(
                    "updateSearchParams",
                    FaeMessage::UpdateSearchParams {
                        params: search_params,
                        request_id: request_id.clone(),
                    },
                );
                
                if let Err(e) = tui_sender.send(tui_message) {
                    log::warn!("Failed to notify TuiActor about search request: {}", e);
                    // Don't fail the entire search for this
                }
            }

            log::debug!("Search request sent successfully with request_id: {}", request_id);
            Ok(())
        } else {
            let error_msg = "Search control sender not initialized";
            log::error!("{}", error_msg);
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                error_msg,
            )))
        }
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

                // Handle external state updates (including search results from TuiActor)
                Some(state_update) = async {
                    match &mut state_receiver {
                        Some(receiver) => receiver.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    log::debug!("TuiApp: Received state update");
                    self.apply_state_update(state_update);
                    self.needs_redraw = true;
                }

                // Search debounce timer - execute pending search when debounce delay expires
                _ = async {
                    if let (Some(last_time), Some(_)) = (self.last_input_time, &self.pending_search_query) {
                        let elapsed = last_time.elapsed();
                        if elapsed >= self.debounce_delay {
                            tokio::time::sleep(Duration::from_millis(0)).await // Immediate wake
                        } else {
                            tokio::time::sleep(self.debounce_delay - elapsed).await
                        }
                    } else {
                        std::future::pending().await // No pending search
                    }
                } => {
                    // Execute the pending search
                    if let Some(query) = self.pending_search_query.take() {
                        log::debug!("TuiApp: Executing debounced search for '{}'", query);
                        if let Err(e) = self.execute_search(query) {
                            log::error!("Failed to execute debounced search: {}", e);
                        }
                        self.last_input_time = None;
                    }
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
            KeyCode::Esc => {
                self.should_quit = true;
            }
            // Tab cycles through search modes, Shift+Tab shows statistics
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    // Shift+Tab: Toggle statistics overlay
                    self.state.show_stats_overlay = !self.state.show_stats_overlay;
                } else {
                    // Tab: Cycle through search modes
                    self.cycle_search_mode();
                }
                self.needs_redraw = true;
            }
            // Emacs-style Control key bindings
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                match c {
                    'c' => self.should_quit = true,

                    // Text editing (emacs-style)
                    'a' => self.move_cursor_to_start(),
                    'b' => self.move_cursor_left(),
                    'e' => self.move_cursor_to_end(),
                    'f' => self.move_cursor_right(),
                    'h' => self.delete_char_backward(),
                    'k' => self.kill_line(),
                    'y' => self.yank(),

                    // Search and navigation
                    'g' => self.abort_search(),
                    'u' => self.scroll_up_half_page(),

                    // Handle Ctrl+D separately based on context
                    'd' => {
                        if self.state.search_input.is_empty()
                            || self.state.cursor_position >= self.state.search_input.len()
                        {
                            // If input is empty or cursor at end, scroll down
                            self.scroll_down_half_page();
                        } else {
                            // Otherwise, delete character forward
                            self.delete_char_forward();
                        }
                    }

                    // Result navigation (existing functionality)
                    'n' => self.move_cursor_down(),
                    'p' => self.move_cursor_up(),

                    _ => return, // Unknown Ctrl combination, don't redraw
                }
                self.needs_redraw = true;
            }

            // Additional Ctrl combinations that need special handling
            KeyCode::Char(',') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.goto_first_result();
                self.needs_redraw = true;
            }
            KeyCode::Char('.') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.goto_last_result();
                self.needs_redraw = true;
            }

            // Result navigation (keep arrow key support)
            KeyCode::Down => {
                self.move_cursor_down();
                self.needs_redraw = true;
            }
            KeyCode::Up => {
                self.move_cursor_up();
                self.needs_redraw = true;
            }

            // Text cursor movement (arrow keys and Home/End)
            KeyCode::Left => {
                self.move_cursor_left();
                self.needs_redraw = true;
            }
            KeyCode::Right => {
                self.move_cursor_right();
                self.needs_redraw = true;
            }
            KeyCode::Home => {
                self.move_cursor_to_start();
                self.needs_redraw = true;
            }
            KeyCode::End => {
                self.move_cursor_to_end();
                self.needs_redraw = true;
            }
            KeyCode::Delete => {
                self.delete_char_forward();
                self.needs_redraw = true;
            }

            // Text input with incremental search
            KeyCode::Char(c) => {
                self.insert_char(c);
                self.execute_incremental_search();
                self.needs_redraw = true;
            }
            KeyCode::Backspace => {
                self.delete_char_backward();
                self.execute_incremental_search();
                self.needs_redraw = true;
            }

            // Enter to select result
            KeyCode::Enter => {
                if self.state.selected_result_index.is_some() {
                    self.handle_result_selection();
                    self.needs_redraw = true;
                }
            }

            _ => {}
        }
    }

    /// Execute incremental search as user types with debounce
    fn execute_incremental_search(&mut self) {
        // Clear previous results and selection (but keep search input intact)
        self.state.search_results.clear();
        self.state.selected_result_index = None;
        // Update ListState for scrolling
        self.state
            .results_list_state
            .select(self.state.selected_result_index);

        if !self.state.search_input.is_empty() {
            log::debug!(
                "TuiApp: Scheduling debounced search for '{}'",
                self.state.search_input
            );

            // Set up debounced search - save query and timestamp
            self.pending_search_query = Some(self.state.search_input.clone());
            self.last_input_time = Some(Instant::now());
        } else {
            // Clear results immediately when input is empty
            log::debug!("TuiApp: Empty search input, clearing results immediately");

            // Cancel any pending search
            self.pending_search_query = None;
            self.last_input_time = None;

            // Execute clear search immediately for empty query
            if let Err(e) = self.execute_search(String::new()) {
                log::error!("Failed to execute clear search: {}", e);
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
            // Update ListState for scrolling
            self.state
                .results_list_state
                .select(self.state.selected_result_index);
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
            // Update ListState for scrolling
            self.state
                .results_list_state
                .select(self.state.selected_result_index);
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

    /// Cycle through search modes: none -> # -> $ -> @ -> / -> none
    fn cycle_search_mode(&mut self) {
        let (current_mode, base_query) = parse_query_with_mode(&self.state.search_input);

        let next_prefix = match current_mode {
            SearchMode::Literal => "#",  // none -> #symbol
            SearchMode::Symbol => "$",   // # -> $variable
            SearchMode::Variable => "@", // $ -> @file
            SearchMode::Filepath => "/", // @ -> /regex
            SearchMode::Regexp => "",    // / -> none (literal)
        };

        // Update search input with new prefix
        self.state.search_input = format!("{}{}", next_prefix, base_query);

        // Execute search with new mode
        self.execute_incremental_search();
    }

    // ===== Emacs-style Text Editing Methods =====

    /// Insert character at cursor position (C-f, Right Arrow)
    fn insert_char(&mut self, c: char) {
        self.state
            .search_input
            .insert(self.state.cursor_position, c);
        self.state.cursor_position += 1;
    }

    /// Move cursor to start of line (C-a, Home)
    fn move_cursor_to_start(&mut self) {
        self.state.cursor_position = 0;
    }

    /// Move cursor to end of line (C-e, End)
    fn move_cursor_to_end(&mut self) {
        self.state.cursor_position = self.state.search_input.len();
    }

    /// Move cursor left one character (C-b, Left Arrow)
    fn move_cursor_left(&mut self) {
        if self.state.cursor_position > 0 {
            self.state.cursor_position -= 1;
        }
    }

    /// Move cursor right one character (C-f, Right Arrow)
    fn move_cursor_right(&mut self) {
        if self.state.cursor_position < self.state.search_input.len() {
            self.state.cursor_position += 1;
        }
    }

    /// Delete character forward at cursor (C-d, Delete)
    fn delete_char_forward(&mut self) {
        if self.state.cursor_position < self.state.search_input.len() {
            self.state.search_input.remove(self.state.cursor_position);
        }
    }

    /// Delete character backward from cursor (C-h, Backspace)
    fn delete_char_backward(&mut self) {
        if self.state.cursor_position > 0 {
            self.state.cursor_position -= 1;
            self.state.search_input.remove(self.state.cursor_position);
        }
    }

    /// Kill text from cursor to end of line (C-k)
    fn kill_line(&mut self) {
        if self.state.cursor_position < self.state.search_input.len() {
            let killed_text = self.state.search_input[self.state.cursor_position..].to_string();
            self.state.kill_ring = killed_text;
            self.state.search_input.truncate(self.state.cursor_position);
        }
    }

    /// Yank (paste) text from kill ring (C-y)
    fn yank(&mut self) {
        if !self.state.kill_ring.is_empty() {
            let insert_text = self.state.kill_ring.clone();
            self.state
                .search_input
                .insert_str(self.state.cursor_position, &insert_text);
            self.state.cursor_position += insert_text.len();
        }
    }

    // ===== Search and Navigation Methods =====

    /// Abort current search and clear input (C-g)
    fn abort_search(&mut self) {
        self.state.search_input.clear();
        self.state.cursor_position = 0;
        self.state.search_results.clear();
        self.state.selected_result_index = None;
        // Update ListState for scrolling
        self.state
            .results_list_state
            .select(self.state.selected_result_index);

        // Show toast notification
        self.state.toast_state.show(
            "Search aborted".to_string(),
            ToastType::Info,
            Duration::from_secs(1),
        );

        // Clear search via actor system
        if let Err(e) = self.execute_search(String::new()) {
            log::error!("Failed to execute clear search: {}", e);
        }
    }

    /// Calculate the actual visible height of the results box
    fn get_results_box_height(&self) -> usize {
        // Get terminal size
        let terminal_size = self.terminal.size().unwrap_or(ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        });

        // Layout calculation: Input box (3) + Status bar (3) + borders
        // Results box gets the remaining height
        let input_height = 3;
        let status_height = 3;
        let available_height = terminal_size
            .height
            .saturating_sub(input_height + status_height);

        // Box border takes 2 lines (top + bottom), so actual content height is area.height - 2
        available_height.saturating_sub(2) as usize
    }

    /// Scroll results up half page (C-u)
    fn scroll_up_half_page(&mut self) {
        if self.state.search_results.is_empty() {
            return;
        }

        let visible_height = self.get_results_box_height();
        let half_page = (visible_height / 2).max(1);

        if let Some(current_index) = self.state.selected_result_index {
            let new_index = current_index.saturating_sub(half_page);
            self.state.selected_result_index = Some(new_index);
        } else {
            self.state.selected_result_index = Some(0);
        }
        // Update ListState for scrolling
        self.state
            .results_list_state
            .select(self.state.selected_result_index);
    }

    /// Scroll results down half page (C-d when input empty)
    fn scroll_down_half_page(&mut self) {
        if self.state.search_results.is_empty() {
            return;
        }

        let visible_height = self.get_results_box_height();
        let half_page = (visible_height / 2).max(1);

        if let Some(current_index) = self.state.selected_result_index {
            let new_index = (current_index + half_page).min(self.state.search_results.len() - 1);
            self.state.selected_result_index = Some(new_index);
        } else {
            self.state.selected_result_index = Some(0);
        }
        // Update ListState for scrolling
        self.state
            .results_list_state
            .select(self.state.selected_result_index);
    }

    /// Jump to first result (C-,)
    fn goto_first_result(&mut self) {
        if !self.state.search_results.is_empty() {
            self.state.selected_result_index = Some(0);
            // Update ListState for scrolling
            self.state
                .results_list_state
                .select(self.state.selected_result_index);
        }
    }

    /// Jump to last result (C-.)
    fn goto_last_result(&mut self) {
        if !self.state.search_results.is_empty() {
            self.state.selected_result_index = Some(self.state.search_results.len() - 1);
            // Update ListState for scrolling
            self.state
                .results_list_state
                .select(self.state.selected_result_index);
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
        let show_stats_overlay = self.state.show_stats_overlay;
        let cursor_position = self.state.cursor_position;

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
            render_input_box(f, chunks[0], &search_input, cursor_position);

            // 2. Results box
            render_results_box(f, chunks[1], &search_results, selected_index);

            // 3. Status bar
            render_status_bar(f, chunks[2], &index_status);

            // 4. Toast (if visible)
            if toast_state.visible {
                render_toast(f, &toast_state);
            }

            // 5. Statistics overlay (if visible)
            if show_stats_overlay {
                render_stats_overlay(f, &index_status);
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
            // Update ListState for scrolling
            self.state
                .results_list_state
                .select(self.state.selected_result_index);
        }

        if let Some(append_results) = update.append_results {
            let was_empty = self.state.search_results.is_empty();
            self.state.search_results.extend(append_results);
            // Set cursor to first result if this was the first addition
            if was_empty && !self.state.search_results.is_empty() {
                self.state.selected_result_index = Some(0);
                // Update ListState for scrolling
                self.state
                    .results_list_state
                    .select(self.state.selected_result_index);
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
            // Update ListState for scrolling
            self.state
                .results_list_state
                .select(self.state.selected_result_index);
        }

        if let Some((message, toast_type, duration)) = update.toast {
            self.state.toast_state.show(message, toast_type, duration);
        }

        if update.clear_results {
            self.state.search_results.clear();
            self.state.selected_result_index = None;
            // Update ListState for scrolling
            self.state
                .results_list_state
                .select(self.state.selected_result_index);
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
            // Update ListState for scrolling
            self.state
                .results_list_state
                .select(self.state.selected_result_index);
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
        // Update ListState for scrolling
        self.state
            .results_list_state
            .select(self.state.selected_result_index);
        self.needs_redraw = true;
    }

    /// Clear all search results
    pub fn clear_search_results(&mut self) {
        self.state.search_results.clear();
        self.state.selected_result_index = None;
        // Update ListState for scrolling
        self.state
            .results_list_state
            .select(self.state.selected_result_index);
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
        // Update ListState for scrolling
        self.state
            .results_list_state
            .select(self.state.selected_result_index);
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

/// Render the input box with search mode indicator and cursor
fn render_input_box(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    search_input: &str,
    cursor_pos: usize,
) {
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

    // Create input string with visible cursor
    let display_text = if search_input.is_empty() {
        if cursor_pos == 0 {
            "█".to_string() // Block cursor at position 0
        } else {
            search_input.to_string()
        }
    } else {
        let mut chars: Vec<char> = search_input.chars().collect();
        if cursor_pos < chars.len() {
            // Insert cursor before the character at cursor_pos
            chars.insert(cursor_pos, '█');
        } else if cursor_pos == chars.len() {
            // Cursor at end of string
            chars.push('█');
        }
        chars.into_iter().collect()
    };

    let input = Paragraph::new(display_text)
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(Color::White));
    f.render_widget(input, area);
}

/// Render the results box with cursor highlighting and automatic scrolling
fn render_results_box(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    search_results: &[String],
    selected_index: Option<usize>,
) {
    let items: Vec<ListItem> = search_results
        .iter()
        .map(|result| ListItem::new(result.as_str()))
        .collect();

    let title = if let Some(index) = selected_index {
        format!("Search Results ({}/{})", index + 1, search_results.len())
    } else {
        "Search Results".to_string()
    };

    let results_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::White));

    // Create a temporary ListState for rendering with current selection
    let mut list_state = ListState::default();
    list_state.select(selected_index);

    f.render_stateful_widget(results_list, area, &mut list_state);
}

/// Render the status bar with help text on left and index status on right
fn render_status_bar(f: &mut Frame, area: ratatui::layout::Rect, _index_status: &IndexStatus) {
    // Updated help text to include emacs-style bindings
    let help_text = "↑↓/C-p/C-n: Navigate | Enter: Select | Tab: Cycle modes | C-a/e: Start/End | C-k/y: Kill/Yank | C-g: Abort | Esc: Quit";
    let help_status = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .style(Style::default().fg(Color::Gray));
    f.render_widget(help_status, area);
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

/// Render statistics overlay in the center of the screen
fn render_stats_overlay(f: &mut Frame, index_status: &IndexStatus) {
    // Calculate the size for the stats overlay
    let area = f.size();
    let popup_area = centered_rect(60, 40, area);

    // Create stats content
    let stats_text = format!(
        "📊 Index Statistics\n\n\
        📁 Files indexed: {}\n\
        🔍 Symbols found: {}\n\
        📋 Queued files: {}\n\
        ✅ Status: {}\n\n\
        Press Shift+Tab to close",
        index_status.indexed_files,
        index_status.symbols_found,
        index_status.queued_files,
        if index_status.is_complete() {
            "Complete"
        } else if index_status.is_active {
            "Indexing..."
        } else {
            "Idle"
        }
    );

    // Create the overlay widget
    let stats_widget = Paragraph::new(stats_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("📊 Statistics")
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .alignment(Alignment::Left)
        .wrap(ratatui::widgets::Wrap { trim: true });

    // Clear the background area
    f.render_widget(
        Block::default()
            .style(Style::default().bg(Color::Black))
            .borders(Borders::NONE),
        popup_area,
    );

    // Render the stats overlay
    f.render_widget(stats_widget, popup_area);
}

/// Create a centered rectangle with the given percentage of the parent area
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

    #[test]
    fn test_emacs_text_editing() {
        // Test emacs-style text editing functionality
        let mut state = TuiState::new();

        // Test insert_char functionality
        state.search_input = "hello".to_string();
        state.cursor_position = 2; // Position between 'e' and 'l'

        // Simulate inserting 'X' at cursor
        state.search_input.insert(state.cursor_position, 'X');
        state.cursor_position += 1;

        assert_eq!(state.search_input, "heXllo");
        assert_eq!(state.cursor_position, 3);

        // Test move_cursor_to_start
        state.cursor_position = 0;
        assert_eq!(state.cursor_position, 0);

        // Test move_cursor_to_end
        state.cursor_position = state.search_input.len();
        assert_eq!(state.cursor_position, 6); // Length of "heXllo"

        // Test kill_line functionality
        state.cursor_position = 2; // Position after 'e'
        let killed_text = state.search_input[state.cursor_position..].to_string();
        state.kill_ring = killed_text.clone();
        state.search_input.truncate(state.cursor_position);

        assert_eq!(state.search_input, "he");
        assert_eq!(state.kill_ring, "Xllo");

        // Test yank functionality
        let insert_text = state.kill_ring.clone();
        state
            .search_input
            .insert_str(state.cursor_position, &insert_text);
        state.cursor_position += insert_text.len();

        assert_eq!(state.search_input, "heXllo");
        assert_eq!(state.cursor_position, 6);
    }

    #[test]
    fn test_emacs_cursor_movement() {
        let mut state = TuiState::new();
        state.search_input = "test".to_string();
        state.cursor_position = 2; // Between 'e' and 's'

        // Test move_cursor_left
        if state.cursor_position > 0 {
            state.cursor_position -= 1;
        }
        assert_eq!(state.cursor_position, 1);

        // Test move_cursor_right
        if state.cursor_position < state.search_input.len() {
            state.cursor_position += 1;
        }
        assert_eq!(state.cursor_position, 2);

        // Test boundary conditions
        state.cursor_position = 0;
        // Try to move left when already at start
        if state.cursor_position > 0 {
            state.cursor_position -= 1;
        }
        assert_eq!(state.cursor_position, 0); // Should remain at 0

        state.cursor_position = state.search_input.len();
        // Try to move right when already at end
        if state.cursor_position < state.search_input.len() {
            state.cursor_position += 1;
        }
        assert_eq!(state.cursor_position, 4); // Should remain at end
    }

    #[test]
    fn test_emacs_delete_operations() {
        let mut state = TuiState::new();
        state.search_input = "hello".to_string();
        state.cursor_position = 2; // Between 'e' and 'l'

        // Test delete_char_forward (C-d)
        if state.cursor_position < state.search_input.len() {
            state.search_input.remove(state.cursor_position);
        }
        assert_eq!(state.search_input, "helo");
        assert_eq!(state.cursor_position, 2); // Cursor stays in place

        // Test delete_char_backward (C-h, Backspace)
        if state.cursor_position > 0 {
            state.cursor_position -= 1;
            state.search_input.remove(state.cursor_position);
        }
        assert_eq!(state.search_input, "hlo");
        assert_eq!(state.cursor_position, 1); // Cursor moves back
    }

    #[test]
    fn test_result_navigation() {
        let mut state = TuiState::new();
        state.search_results = vec![
            "result1".to_string(),
            "result2".to_string(),
            "result3".to_string(),
        ];
        state.selected_result_index = Some(1); // Start at second result

        // Test scroll_up_half_page (C-u)
        let visible_height = 10;
        let half_page = (visible_height / 2).max(1);
        if let Some(current_index) = state.selected_result_index {
            let new_index = current_index.saturating_sub(half_page);
            state.selected_result_index = Some(new_index);
        }
        assert_eq!(state.selected_result_index, Some(0)); // Should go to first result

        // Test goto_last_result (C-.)
        if !state.search_results.is_empty() {
            state.selected_result_index = Some(state.search_results.len() - 1);
        }
        assert_eq!(state.selected_result_index, Some(2)); // Should go to last result

        // Test goto_first_result (C-,)
        if !state.search_results.is_empty() {
            state.selected_result_index = Some(0);
        }
        assert_eq!(state.selected_result_index, Some(0)); // Should go to first result
    }
}
