use crate::{
    backend::{BackendEvent, SearchBackend, UserCommand},
    tui_state::{TuiAction, TuiInput, TuiState},
    types::{SearchMode, SearchResult},
};
use anyhow::Result;
use crossterm::event::KeyCode;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Programmatic TUI simulator for testing UI behavior without actual TUI
/// This allows us to test the complete TUI workflow programmatically
pub struct TuiSimulator {
    state: TuiState,
    command_sender: mpsc::Sender<UserCommand>,
    event_receiver: mpsc::Receiver<BackendEvent>,
    backend_thread: Option<thread::JoinHandle<()>>,
}

impl TuiSimulator {
    /// Create a new TUI simulator with backend (starts backend thread)
    pub fn new(verbose: bool, respect_gitignore: bool) -> Result<Self> {
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

    /// Create a new TUI simulator without starting backend thread
    /// Use this for testing scenarios where backend control is needed
    pub fn new_without_backend_thread(
        verbose: bool,
        respect_gitignore: bool,
    ) -> Result<(Self, SearchBackend)> {
        let (backend, command_sender, event_receiver) =
            SearchBackend::new(verbose, respect_gitignore);

        let simulator = Self {
            state: TuiState::new(),
            command_sender,
            event_receiver,
            backend_thread: None,
        };

        Ok((simulator, backend))
    }

    /// Initialize simulator with directory indexing
    pub fn initialize(&mut self, directory: PathBuf) -> Result<()> {
        self.send_command(UserCommand::StartIndexing { directory })?;
        Ok(())
    }

    /// Enable file watching
    pub fn enable_file_watching(&mut self) -> Result<()> {
        self.state.watch_enabled = true;
        self.send_command(UserCommand::EnableFileWatching)?;
        Ok(())
    }

    /// Send a command to backend
    pub fn send_command(&mut self, command: UserCommand) -> Result<()> {
        self.command_sender
            .send(command)
            .map_err(|e| anyhow::anyhow!("Failed to send command: {}", e))?;
        Ok(())
    }

    /// Wait for and process next backend event
    pub fn wait_for_event(&mut self) -> Result<BackendEvent> {
        let event = self
            .event_receiver
            .recv()
            .map_err(|e| anyhow::anyhow!("Failed to receive event: {}", e))?;

        self.state.apply_backend_event(event.clone());
        Ok(event)
    }

    /// Try to receive and process backend event (non-blocking)
    pub fn try_process_event(&mut self) -> Result<Option<BackendEvent>> {
        match self.event_receiver.try_recv() {
            Ok(event) => {
                self.state.apply_backend_event(event.clone());
                Ok(Some(event))
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(anyhow::anyhow!("Backend disconnected")),
        }
    }

    /// Process all pending events
    pub fn process_all_events(&mut self) -> Result<Vec<BackendEvent>> {
        let mut events = Vec::new();

        while let Some(event) = self.try_process_event()? {
            events.push(event);
        }

        Ok(events)
    }

    /// Get current state (read-only)
    pub fn get_state(&self) -> &TuiState {
        &self.state
    }

    /// Simulate typing text
    pub fn simulate_typing(&mut self, text: &str) -> Result<()> {
        for c in text.chars() {
            self.simulate_input(TuiInput::TypeChar(c))?;
        }
        Ok(())
    }

    /// Simulate a key press
    pub fn simulate_key_press(&mut self, key: KeyCode) -> Result<()> {
        let input = match key {
            KeyCode::Esc => TuiInput::Quit,
            KeyCode::Char('?') => TuiInput::ToggleHelp,
            KeyCode::Up => TuiInput::NavigateUp,
            KeyCode::Down => TuiInput::NavigateDown,
            KeyCode::Enter => TuiInput::Select,
            KeyCode::Backspace => TuiInput::Backspace,
            KeyCode::Char(c) => TuiInput::TypeChar(c),
            _ => return Ok(()), // Ignore other keys
        };

        self.simulate_input(input)
    }

    /// Simulate input and handle resulting actions
    fn simulate_input(&mut self, input: TuiInput) -> Result<()> {
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
                    // In real TUI, this would copy to clipboard
                    // For testing, we can store it or just log it
                    println!("Would copy to clipboard: {}", text);
                }
            }
        }

        Ok(())
    }

    /// Wait for indexing to complete
    pub fn wait_for_indexing_complete(&mut self) -> Result<Duration> {
        loop {
            let event = self.wait_for_event()?;
            match event {
                BackendEvent::IndexingComplete { duration, .. } => {
                    return Ok(duration);
                }
                BackendEvent::Error { message } => {
                    return Err(anyhow::anyhow!("Indexing failed: {}", message));
                }
                _ => {
                    // Continue waiting
                }
            }
        }
    }

    /// Perform search and wait for results
    pub fn search_and_wait(&mut self, query: &str) -> Result<Vec<SearchResult>> {
        // Simulate typing the query
        self.simulate_typing(query)?;

        // Wait for search results - we need to wait for all typing events to complete
        // Since typing triggers multiple search events, we should wait for the final one

        loop {
            let event = self.wait_for_event()?;
            match event {
                BackendEvent::SearchResults {
                    results,
                    query: search_query,
                } => {
                    // If the search query matches our expected query, we have the final results
                    if search_query == query {
                        return Ok(results);
                    }
                }
                BackendEvent::Error { message } => {
                    return Err(anyhow::anyhow!("Search failed: {}", message));
                }
                _ => {
                    // Continue waiting
                }
            }
        }
    }

    /// Wait for specific event type with timeout
    pub fn wait_for_event_timeout(&mut self, timeout: Duration) -> Result<Option<BackendEvent>> {
        let start = std::time::Instant::now();

        loop {
            if let Some(event) = self.try_process_event()? {
                return Ok(Some(event));
            }

            if start.elapsed() > timeout {
                return Ok(None);
            }

            thread::sleep(Duration::from_millis(10));
        }
    }

    /// Get current search results
    pub fn get_search_results(&self) -> &[SearchResult] {
        &self.state.current_results
    }

    /// Get current query
    pub fn get_query(&self) -> &str {
        &self.state.query
    }

    /// Get current search mode
    pub fn get_search_mode(&self) -> &SearchMode {
        &self.state.current_search_mode
    }

    /// Check if currently indexing
    pub fn is_indexing(&self) -> bool {
        self.state.is_indexing
    }

    /// Check if should quit
    pub fn should_quit(&self) -> bool {
        self.state.should_quit
    }

    /// Get status message
    pub fn get_status_message(&self) -> &str {
        &self.state.status_message
    }

    /// Simulate navigation to specific index
    pub fn navigate_to(&mut self, index: usize) -> Result<()> {
        while self.state.selected_index < index {
            self.simulate_input(TuiInput::NavigateDown)?;
        }
        while self.state.selected_index > index {
            self.simulate_input(TuiInput::NavigateUp)?;
        }
        Ok(())
    }

    /// Simulate selecting current item
    pub fn select_current(&mut self) -> Result<Option<String>> {
        if let Some(result) = self.state.current_results.get(self.state.selected_index) {
            let location = format!(
                "{}:{}:{}",
                result.symbol.file.display(),
                result.symbol.line,
                result.symbol.column
            );
            self.simulate_input(TuiInput::Select)?;
            Ok(Some(location))
        } else {
            Ok(None)
        }
    }
}

impl Drop for TuiSimulator {
    fn drop(&mut self) {
        // Send quit command to clean up backend
        let _ = self.command_sender.send(UserCommand::Quit);

        // Wait for backend thread to finish
        if let Some(handle) = self.backend_thread.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_project() -> Result<TempDir> {
        let temp_dir = TempDir::new()?;

        // Create a simple test file
        let test_file = temp_dir.path().join("test.ts");
        fs::write(
            &test_file,
            r#"
class TestClass {
    private name: string;
    
    constructor(name: string) {
        this.name = name;
    }
    
    public getName(): string {
        return this.name;
    }
}

function testFunction() {
    const test = new TestClass("test");
    return test.getName();
}
"#,
        )?;

        Ok(temp_dir)
    }

    #[test]
    fn test_simulator_basic_workflow() -> Result<()> {
        let temp_dir = create_test_project()?;
        let mut simulator = TuiSimulator::new(false, true)?;

        // Initialize with test directory
        simulator.initialize(temp_dir.path().to_path_buf())?;

        // Wait for indexing to complete
        let duration = simulator.wait_for_indexing_complete()?;
        assert!(duration.as_millis() < 5000); // Should complete quickly

        // Check that symbols were found
        assert!(!simulator.get_state().symbols.is_empty());

        Ok(())
    }

    #[test]
    fn test_simulator_search_workflow() -> Result<()> {
        let temp_dir = create_test_project()?;
        let mut simulator = TuiSimulator::new(false, true)?;

        // Initialize and wait for indexing
        simulator.initialize(temp_dir.path().to_path_buf())?;
        simulator.wait_for_indexing_complete()?;

        // Search for a symbol
        let results = simulator.search_and_wait("#TestClass")?;
        assert!(!results.is_empty());

        // Check search mode was detected correctly
        assert_eq!(simulator.get_search_mode().name, "Symbol");

        Ok(())
    }

    #[test]
    fn test_simulator_navigation() -> Result<()> {
        let temp_dir = create_test_project()?;
        let mut simulator = TuiSimulator::new(false, true)?;

        // Initialize and search
        simulator.initialize(temp_dir.path().to_path_buf())?;
        simulator.wait_for_indexing_complete()?;
        simulator.search_and_wait("test")?;

        // Test navigation
        let initial_index = simulator.get_state().selected_index;
        simulator.simulate_key_press(KeyCode::Down)?;
        assert!(simulator.get_state().selected_index > initial_index);

        simulator.simulate_key_press(KeyCode::Up)?;
        assert_eq!(simulator.get_state().selected_index, initial_index);

        Ok(())
    }
}
