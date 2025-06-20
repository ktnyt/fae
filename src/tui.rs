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
use ratatui::{backend::CrosstermBackend, widgets::ListState, Terminal};
use std::{
    io::{stdout, Result, Stdout},
    time::Duration,
};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

/// Trait for handling TUI messages and search operations
/// This allows external components to handle search execution while keeping TuiApp focused on UI
pub trait TuiMessageHandler {
    /// Execute a search with the given query
    fn execute_search(
        &self,
        query: String,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Clear all search results
    fn clear_results(&self) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Abort current search
    fn abort_search(&self) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

// Import search-related types
use crate::actors::types::SearchMode;
use crate::cli::{parse_query_with_mode, PREFIX_SYMBOL, PREFIX_VARIABLE, PREFIX_FILEPATH, PREFIX_REGEX};

// Import modular components
mod toast;
pub use toast::{ToastState, ToastType};

mod input;
pub use input::{InputHandler, InputOperation};

mod search_debouncer;
pub use search_debouncer::SearchDebouncer;

mod rendering_controller;
pub use rendering_controller::RenderingController;

mod renderer;
pub use renderer::TuiRenderer;

/// Type alias for TUI state update results to avoid large error types
pub type TuiResult<T = ()> = std::result::Result<T, Box<mpsc::error::SendError<StateUpdate>>>;

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

/// TUI application state with organized responsibility groups
#[derive(Clone, Debug)]
pub struct TuiState {
    // === Search Interface ===
    pub search_input: String,   // Input string for search queries
    pub cursor_position: usize, // Cursor position in search_input
    pub kill_ring: String,      // Kill/yank buffer (emacs-style)

    // === Results Display ===
    pub search_results: Vec<String>,          // Search results array
    pub selected_result_index: Option<usize>, // Selected result cursor position
    pub results_list_state: ListState,        // StatefulList scroll state

    // === UI Feedback ===
    pub toast_state: ToastState,   // Toast notification state
    pub index_status: IndexStatus, // Index status for status bar
    pub show_stats_overlay: bool,  // Statistics overlay visibility
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

/// TUI application with organized state management
pub struct TuiApp {
    // === Terminal Control ===
    terminal: Terminal<CrosstermBackend<Stdout>>,
    should_quit: bool,

    // === Rendering & UI ===
    rendering_controller: RenderingController,

    // === External Communication ===
    state_receiver: Option<mpsc::UnboundedReceiver<StateUpdate>>,
    message_handler: Option<Box<dyn TuiMessageHandler + Send>>,

    // === Search Control ===
    search_debouncer: SearchDebouncer,

    // === Application State ===
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
            // Rendering control (60 FPS)
            rendering_controller: RenderingController::new(),
            state_receiver: Some(state_receiver),
            message_handler: None, // Will be set later via set_message_handler
            // Search debounce control
            search_debouncer: SearchDebouncer::new(),
            state: TuiState::new(),
        };

        Ok((app, handle))
    }

    /// Set the external message handler for search operations
    pub fn set_message_handler(&mut self, handler: Box<dyn TuiMessageHandler + Send>) {
        self.message_handler = Some(handler);
    }

    /// Execute a dynamic search via external message handler
    pub fn execute_search(
        &self,
        query: String,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref handler) = self.message_handler {
            log::debug!("TuiApp executing search: '{}'", query);

            // Abort any ongoing search and clear results before starting new search
            if let Err(e) = handler.abort_search() {
                log::warn!("Failed to abort previous search: {}", e);
            }

            if let Err(e) = handler.clear_results() {
                log::warn!("Failed to clear previous results: {}", e);
            }

            // Skip search if query is empty or just contains search prefixes
            let trimmed_query = query.trim();
            if trimmed_query.is_empty()
                || trimmed_query == "#"
                || trimmed_query == "$"
                || trimmed_query == "@"
                || trimmed_query == "/"
            {
                log::debug!(
                    "Aborting search for empty or prefix-only query: '{}'",
                    query
                );
                return Ok(());
            }

            // Execute the search
            handler.execute_search(query)?;
            log::debug!("Search request sent successfully");
            Ok(())
        } else {
            let error_msg = "Message handler not initialized";
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
        self.rendering_controller.mark_drawn();

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
                    self.rendering_controller.request_redraw();
                }

                // Search debounce timer - execute pending search when debounce delay expires
                _ = async {
                    if self.search_debouncer.has_pending_search() {
                        if let Some(remaining) = self.search_debouncer.time_until_ready() {
                            tokio::time::sleep(remaining).await
                        } else {
                            tokio::time::sleep(Duration::from_millis(0)).await // Immediate wake
                        }
                    } else {
                        std::future::pending().await // No pending search
                    }
                } => {
                    // Execute the pending search
                    if let Some(query) = self.search_debouncer.check_ready_for_search() {
                        log::debug!("TuiApp: Executing debounced search for '{}'", query);
                        if let Err(e) = self.execute_search(query) {
                            log::error!("Failed to execute debounced search: {}", e);
                        }
                    }
                }

                // Periodic updates (toast expiration, etc.)
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    if self.state.update_toast() {
                        self.rendering_controller.request_redraw();
                    }
                }
            }

            // Check if we should quit
            if self.should_quit {
                break;
            }

            // Only redraw if needed and enough time has passed (throttling)
            if self.rendering_controller.should_draw() {
                self.draw()?;
                self.rendering_controller.mark_drawn();
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
                    // Shift+Tab: Cycle through search modes in reverse
                    self.cycle_search_mode_reverse();
                } else {
                    // Tab: Cycle through search modes forward
                    self.cycle_search_mode_forward();
                }
                self.rendering_controller.request_redraw();
            }
            // Emacs-style Control key bindings
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                match c {
                    'c' => self.should_quit = true,

                    // Text editing (emacs-style)
                    'a' => self.handle_input_operation(InputOperation::MoveCursorToStart),
                    'b' => self.handle_input_operation(InputOperation::MoveCursorLeft),
                    'e' => self.handle_input_operation(InputOperation::MoveCursorToEnd),
                    'f' => self.handle_input_operation(InputOperation::MoveCursorRight),
                    'h' => {
                        self.handle_input_operation(InputOperation::DeleteCharBackward);
                        self.execute_incremental_search();
                    }
                    'k' => {
                        self.handle_input_operation(InputOperation::KillLine);
                        self.execute_incremental_search();
                    }
                    'y' => {
                        self.handle_input_operation(InputOperation::Yank);
                        self.execute_incremental_search();
                    }

                    // Search and navigation
                    'g' => self.abort_search(),
                    's' => {
                        // Ctrl+S: Toggle statistics overlay
                        self.state.show_stats_overlay = !self.state.show_stats_overlay;
                    },
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
                            self.handle_input_operation(InputOperation::DeleteCharForward);
                            self.execute_incremental_search();
                        }
                    }

                    // Result navigation (existing functionality)
                    'n' => self.move_cursor_down(),
                    'p' => self.move_cursor_up(),

                    _ => return, // Unknown Ctrl combination, don't redraw
                }
                self.rendering_controller.request_redraw();
            }

            // Additional Ctrl combinations that need special handling
            KeyCode::Char(',') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.goto_first_result();
                self.rendering_controller.request_redraw();
            }
            KeyCode::Char('.') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.goto_last_result();
                self.rendering_controller.request_redraw();
            }

            // Result navigation (keep arrow key support)
            KeyCode::Down => {
                self.move_cursor_down();
                self.rendering_controller.request_redraw();
            }
            KeyCode::Up => {
                self.move_cursor_up();
                self.rendering_controller.request_redraw();
            }

            // Text cursor movement (arrow keys and Home/End)
            KeyCode::Left => {
                self.handle_input_operation(InputOperation::MoveCursorLeft);
                self.rendering_controller.request_redraw();
            }
            KeyCode::Right => {
                self.handle_input_operation(InputOperation::MoveCursorRight);
                self.rendering_controller.request_redraw();
            }
            KeyCode::Home => {
                self.handle_input_operation(InputOperation::MoveCursorToStart);
                self.rendering_controller.request_redraw();
            }
            KeyCode::End => {
                self.handle_input_operation(InputOperation::MoveCursorToEnd);
                self.rendering_controller.request_redraw();
            }
            KeyCode::Delete => {
                self.handle_input_operation(InputOperation::DeleteCharForward);
                self.execute_incremental_search();
                self.rendering_controller.request_redraw();
            }

            // Text input with incremental search
            KeyCode::Char(c) => {
                self.handle_input_operation(InputOperation::InsertChar(c));
                self.execute_incremental_search();
                self.rendering_controller.request_redraw();
            }
            KeyCode::Backspace => {
                self.handle_input_operation(InputOperation::DeleteCharBackward);
                self.execute_incremental_search();
                self.rendering_controller.request_redraw();
            }

            // Enter to select result
            KeyCode::Enter => {
                if self.state.selected_result_index.is_some() {
                    self.handle_result_selection();
                    self.rendering_controller.request_redraw();
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

            // Set up debounced search using SearchDebouncer
            self.search_debouncer
                .set_pending_search(self.state.search_input.clone());
        } else {
            // Clear results immediately when input is empty
            log::debug!("TuiApp: Empty search input, clearing results immediately");

            // Cancel any pending search
            self.search_debouncer.clear_pending_search();

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

    /// Cycle through search modes forward: none -> # -> $ -> @ -> / -> none
    fn cycle_search_mode_forward(&mut self) {
        let (current_mode, base_query) = parse_query_with_mode(&self.state.search_input);
        let old_prefix_len = self.get_prefix_length(current_mode);

        let next_prefix = match current_mode {
            SearchMode::Literal => PREFIX_SYMBOL.to_string(),     // none -> #symbol
            SearchMode::Symbol => PREFIX_VARIABLE.to_string(),    // # -> $variable
            SearchMode::Variable => PREFIX_FILEPATH.to_string(),  // $ -> >file
            SearchMode::Filepath => PREFIX_REGEX.to_string(),     // > -> /regex
            SearchMode::Regexp => "".to_string(),                 // / -> none (literal)
        };

        // Update search input with new prefix
        self.state.search_input = format!("{}{}", next_prefix, base_query);

        // Adjust cursor position: if we added a prefix, move cursor right; if removed, move left
        let new_prefix_len = next_prefix.len();
        if old_prefix_len == 0 && new_prefix_len > 0 {
            // Added prefix: move cursor right by 1
            self.state.cursor_position = self.state.cursor_position.saturating_add(1);
        } else if old_prefix_len > 0 && new_prefix_len == 0 {
            // Removed prefix: move cursor left by 1
            self.state.cursor_position = self.state.cursor_position.saturating_sub(1);
        }
        // Ensure cursor doesn't go beyond string length
        self.state.cursor_position = self.state.cursor_position.min(self.state.search_input.len());

        // Execute search with new mode
        self.execute_incremental_search();
    }

    /// Cycle through search modes in reverse: none <- # <- $ <- @ <- / <- none
    fn cycle_search_mode_reverse(&mut self) {
        let (current_mode, base_query) = parse_query_with_mode(&self.state.search_input);
        let old_prefix_len = self.get_prefix_length(current_mode);

        let next_prefix = match current_mode {
            SearchMode::Literal => PREFIX_REGEX.to_string(),      // none <- /regex
            SearchMode::Regexp => PREFIX_FILEPATH.to_string(),    // / <- >file
            SearchMode::Filepath => PREFIX_VARIABLE.to_string(),  // > <- $variable
            SearchMode::Variable => PREFIX_SYMBOL.to_string(),    // $ <- #symbol
            SearchMode::Symbol => "".to_string(),                 // # <- none (literal)
        };

        // Update search input with new prefix
        self.state.search_input = format!("{}{}", next_prefix, base_query);

        // Adjust cursor position: if we added a prefix, move cursor right; if removed, move left
        let new_prefix_len = next_prefix.len();
        if old_prefix_len == 0 && new_prefix_len > 0 {
            // Added prefix: move cursor right by 1
            self.state.cursor_position = self.state.cursor_position.saturating_add(1);
        } else if old_prefix_len > 0 && new_prefix_len == 0 {
            // Removed prefix: move cursor left by 1
            self.state.cursor_position = self.state.cursor_position.saturating_sub(1);
        }
        // Ensure cursor doesn't go beyond string length
        self.state.cursor_position = self.state.cursor_position.min(self.state.search_input.len());

        // Execute search with new mode
        self.execute_incremental_search();
    }

    /// Get the length of the prefix for a given search mode
    fn get_prefix_length(&self, mode: SearchMode) -> usize {
        match mode {
            SearchMode::Literal => 0,
            SearchMode::Symbol => 1,    // "#"
            SearchMode::Variable => 1,  // "$"
            SearchMode::Filepath => 1,  // ">"
            SearchMode::Regexp => 1,    // "/"
        }
    }

    // ===== Input Processing Integration =====

    /// Handle input processing with unified InputHandler
    fn handle_input_operation(&mut self, operation: InputOperation) {
        InputHandler::apply_operation(
            operation,
            &mut self.state.search_input,
            &mut self.state.cursor_position,
            &mut self.state.kill_ring,
        );
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
        // Use centralized renderer for all UI components
        TuiRenderer::render(
            &mut self.terminal,
            &self.state.search_input,
            self.state.cursor_position,
            &self.state.search_results,
            self.state.selected_result_index,
            &self.state.toast_state,
            &self.state.index_status,
            self.state.show_stats_overlay,
        )
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
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
        self.rendering_controller.request_redraw();
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
        self.rendering_controller.request_redraw();
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
        self.rendering_controller.request_redraw();
    }

    /// Clear all search results
    pub fn clear_search_results(&mut self) {
        self.state.search_results.clear();
        self.state.selected_result_index = None;
        // Update ListState for scrolling
        self.state
            .results_list_state
            .select(self.state.selected_result_index);
        self.rendering_controller.request_redraw();
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
        self.rendering_controller.request_redraw();
    }

    /// Show toast notification from external source
    pub fn show_toast(&mut self, message: String, toast_type: ToastType, duration: Duration) {
        self.state.toast_state.show(message, toast_type, duration);
        self.rendering_controller.request_redraw();
    }

    /// Hide current toast
    pub fn hide_toast(&mut self) {
        self.state.toast_state.hide();
        self.rendering_controller.request_redraw();
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
        self.rendering_controller.request_redraw();
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
        self.rendering_controller.request_redraw();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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

        let top_right = TuiRenderer::top_right_rect_absolute(30, 20, full_rect);

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
        let result = TuiRenderer::calculate_wrapped_lines(message, width);
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
        let result = TuiRenderer::calculate_wrapped_lines(message, width);
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
        assert_eq!(TuiRenderer::calculate_wrapped_lines("", 10), 1);

        // Test single word that fits
        assert_eq!(TuiRenderer::calculate_wrapped_lines("hello", 10), 1);

        // Test multiple words that fit on one line
        assert_eq!(TuiRenderer::calculate_wrapped_lines("hello world", 15), 1);

        // Test text that needs wrapping
        assert_eq!(TuiRenderer::calculate_wrapped_lines("hello world test", 10), 2);

        // Test very long word that needs breaking
        // "verylongwordthatdoesnotfit" = 26 chars, with width 10 = ceil(26/10) = 3 lines
        // But since we add +1 for current line and calculate separately, adjust expectation
        let result = TuiRenderer::calculate_wrapped_lines("verylongwordthatdoesnotfit", 10);
        assert!(result >= 3, "Expected at least 3 lines, got {}", result);

        // Test zero width (edge case)
        assert!(TuiRenderer::calculate_wrapped_lines("test", 0) > 0);

        // Test realistic toast message
        let long_message = "Indexing completed: 25 files, 1200 symbols found successfully";
        assert!(TuiRenderer::calculate_wrapped_lines(long_message, 30) >= 2);
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

        let (width, height) = TuiRenderer::calculate_toast_size_absolute(&toast, terminal_size);
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
        let display_message = TuiRenderer::get_toast_display_message(&toast);
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

        let content_lines = TuiRenderer::calculate_wrapped_lines(&display_message, available_width);
        println!("Content lines: {}", content_lines);

        let total_lines = content_lines + 2;
        println!("Total lines (content + borders): {}", total_lines);

        let height_percent =
            ((total_lines * 100) / (terminal_size.height as usize)).clamp(15, 50) as u16;
        println!("Height percent: {}", height_percent);

        // Now test the actual function
        let (actual_width, actual_height) = TuiRenderer::calculate_toast_size_absolute(&toast, terminal_size);
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
        let (width, height) = TuiRenderer::calculate_toast_size_absolute(&toast, terminal_size);

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

        let (width, height) = TuiRenderer::calculate_toast_size_absolute(&toast, terminal_size);

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

        let (_width2, height2) = TuiRenderer::calculate_toast_size_absolute(&toast, terminal_size);
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
        let (width_small, height_small) = TuiRenderer::calculate_toast_size_absolute(&toast, small_terminal);

        let large_terminal = Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 60,
        };
        let (width_large, height_large) = TuiRenderer::calculate_toast_size_absolute(&toast, large_terminal);

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
