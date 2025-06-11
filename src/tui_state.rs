use crate::{
    backend::{BackendEvent, FileChangeType},
    types::{CodeSymbol, SearchResult, DefaultDisplayStrategy, SearchMode},
};
use std::time::Duration;

/// UI state for TUI application
/// This contains only state information, no rendering logic
#[derive(Debug, Clone)]
pub struct TuiState {
    // Symbol and search data
    pub symbols: Vec<CodeSymbol>,
    pub current_results: Vec<SearchResult>,
    pub selected_index: usize,
    pub query: String,
    pub current_search_mode: SearchMode,
    pub search_modes: Vec<SearchMode>,
    
    // UI state
    pub should_quit: bool,
    pub show_help: bool,
    pub status_message: String,
    pub default_strategy: DefaultDisplayStrategy,
    
    // Indexing state
    pub is_indexing: bool,
    pub indexing_progress: Option<IndexingProgress>,
    
    // File watching state
    pub watch_enabled: bool,
    pub last_updated_file: Option<String>,
}


#[derive(Debug, Clone)]
pub struct IndexingProgress {
    pub processed: usize,
    pub total: usize,
    pub percentage: u32,
}

impl Default for TuiState {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiState {
    pub fn new() -> Self {
        let search_modes = vec![
            SearchMode {
                name: "Content".to_string(),
                prefix: "".to_string(),
                icon: "🔍".to_string(),
            },
            SearchMode {
                name: "Symbol".to_string(),
                prefix: "#".to_string(),
                icon: "🏷️".to_string(),
            },
            SearchMode {
                name: "File".to_string(),
                prefix: ">".to_string(),
                icon: "📁".to_string(),
            },
            SearchMode {
                name: "Regex".to_string(),
                prefix: "/".to_string(),
                icon: "🔧".to_string(),
            },
        ];

        Self {
            symbols: Vec::new(),
            current_results: Vec::new(),
            selected_index: 0,
            query: String::new(),
            current_search_mode: search_modes[0].clone(),
            search_modes,
            should_quit: false,
            show_help: false,
            status_message: "Ready".to_string(),
            default_strategy: DefaultDisplayStrategy::RecentlyModified,
            is_indexing: false,
            indexing_progress: None,
            watch_enabled: false,
            last_updated_file: None,
        }
    }
    
    /// Apply backend event to update state
    pub fn apply_backend_event(&mut self, event: BackendEvent) {
        match event {
            BackendEvent::IndexingProgress { processed, total, symbols } => {
                self.symbols.extend(symbols);
                self.is_indexing = true;
                self.indexing_progress = Some(IndexingProgress {
                    processed,
                    total,
                    percentage: (processed as f32 / total as f32 * 100.0) as u32,
                });
                self.status_message = format!("Indexing progress: {}/{} files ({}%)", 
                                            processed, total, 
                                            (processed as f32 / total as f32 * 100.0) as u32);
                
                // Update current results if we have a query
                if !self.query.trim().is_empty() {
                    self.update_search_results();
                } else {
                    self.show_default_results();
                }
            }
            BackendEvent::IndexingComplete { duration, total_symbols } => {
                self.is_indexing = false;
                self.indexing_progress = None;
                
                let duration_msg = if duration.as_millis() < 1000 {
                    format!("{}ms", duration.as_millis())
                } else {
                    format!("{:.1}s", duration.as_secs_f64())
                };
                
                self.status_message = format!("Indexing complete! Found {} symbols ({})", 
                                            total_symbols, duration_msg);
                
                // Update current results
                if !self.query.trim().is_empty() {
                    self.update_search_results();
                } else {
                    self.show_default_results();
                }
            }
            BackendEvent::FileChanged { file, change_type } => {
                let filename = file.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                
                self.last_updated_file = Some(filename.clone());
                
                self.status_message = match change_type {
                    FileChangeType::Created => format!("File added: {}", filename),
                    FileChangeType::Modified => format!("File modified: {}", filename),
                    FileChangeType::Deleted => format!("File deleted: {}", filename),
                };
                
                // Update current results
                if !self.query.trim().is_empty() {
                    self.update_search_results();
                } else {
                    self.show_default_results();
                }
            }
            BackendEvent::SearchResults { query: _, results } => {
                self.current_results = results;
                self.selected_index = 0;
            }
            BackendEvent::Error { message } => {
                self.status_message = format!("Error: {}", message);
            }
        }
    }
    
    /// Handle user input to update state
    pub fn handle_input(&mut self, input: TuiInput) -> Vec<TuiAction> {
        let mut actions = Vec::new();
        
        match input {
            TuiInput::Quit => {
                self.should_quit = true;
                actions.push(TuiAction::Quit);
            }
            TuiInput::ToggleHelp => {
                self.show_help = !self.show_help;
            }
            TuiInput::NavigateUp => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            TuiInput::NavigateDown => {
                if self.selected_index < self.current_results.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
            }
            TuiInput::Select => {
                if let Some(result) = self.current_results.get(self.selected_index) {
                    let location = format!(
                        "{}:{}:{}",
                        result.symbol.file.display(),
                        result.symbol.line,
                        result.symbol.column
                    );
                    actions.push(TuiAction::CopyToClipboard { text: location });
                    self.query.clear();
                    self.update_search_results();
                }
            }
            TuiInput::TypeChar(c) => {
                self.query.push(c);
                self.current_search_mode = self.detect_search_mode(&self.query);
                actions.push(TuiAction::Search {
                    query: self.query.clone(),
                    mode: self.current_search_mode.clone(),
                });
            }
            TuiInput::Backspace => {
                self.query.pop();
                self.current_search_mode = self.detect_search_mode(&self.query);
                actions.push(TuiAction::Search {
                    query: self.query.clone(),
                    mode: self.current_search_mode.clone(),
                });
            }
        }
        
        actions
    }
    
    pub fn detect_search_mode(&self, query: &str) -> SearchMode {
        if query.starts_with('#') {
            return self.search_modes[1].clone(); // Symbol
        } else if query.starts_with('>') {
            return self.search_modes[2].clone(); // File
        } else if query.starts_with('/') {
            return self.search_modes[3].clone(); // Regex
        }
        self.search_modes[0].clone() // Content (default)
    }
    
    fn update_search_results(&mut self) {
        // This would trigger a search action
        // The actual search is handled by the backend
    }
    
    fn show_default_results(&mut self) {
        // This would show default results based on strategy
        // For now, just clear results if query is empty
        if self.query.trim().is_empty() {
            // Don't clear results here - let backend handle it
        }
    }
    
    /// Get current display mode info for UI
    pub fn get_mode_info(&self) -> String {
        if self.query.is_empty() && self.current_search_mode.name == "Content" {
            format!("{} Recently Edited", self.current_search_mode.icon)
        } else {
            format!("{} {} Search", self.current_search_mode.icon, self.current_search_mode.name)
        }
    }
    
    /// Get status line information
    pub fn get_status_info(&self) -> Vec<StatusSpan> {
        let mut spans = vec![
            StatusSpan::Label("Status: ".to_string()),
            StatusSpan::Text(self.status_message.clone()),
        ];
        
        if self.watch_enabled {
            spans.push(StatusSpan::Success(" | 👁 Watch: ON".to_string()));
        } else {
            spans.push(StatusSpan::Error(" | 👁 Watch: OFF".to_string()));
        }
        
        if let Some(ref last_file) = self.last_updated_file {
            spans.push(StatusSpan::Label(" | Last: ".to_string()));
            spans.push(StatusSpan::Text(last_file.clone()));
        }
        
        spans
    }
}

/// Input events that can be sent to TUI state
#[derive(Debug, Clone)]
pub enum TuiInput {
    Quit,
    ToggleHelp,
    NavigateUp,
    NavigateDown,
    Select,
    TypeChar(char),
    Backspace,
}

/// Actions that should be taken as a result of state changes
#[derive(Debug, Clone)]
pub enum TuiAction {
    Quit,
    Search { query: String, mode: SearchMode },
    CopyToClipboard { text: String },
}

/// Status line span with semantic meaning
#[derive(Debug, Clone)]
pub enum StatusSpan {
    Label(String),
    Text(String),
    Success(String),
    Error(String),
}