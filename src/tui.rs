use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io;
use arboard::Clipboard;

use crate::{
    indexer::TreeSitterIndexer,
    searcher::FuzzySearcher,
    types::{CodeSymbol, DefaultDisplayStrategy, SearchOptions, SearchResult, SymbolType},
};

#[derive(Debug, Clone)]
pub struct SearchMode {
    pub name: String,
    pub prefix: String,
    pub icon: String,
}

pub struct TuiApp {
    pub searcher: Option<FuzzySearcher>,
    pub symbols: Vec<CodeSymbol>,
    pub current_results: Vec<SearchResult>,
    pub selected_index: usize,
    pub query: String,
    pub current_search_mode: SearchMode,
    pub search_modes: Vec<SearchMode>,
    pub should_quit: bool,
    pub show_help: bool,
    pub status_message: String,
    pub default_strategy: DefaultDisplayStrategy,
}

impl TuiApp {
    pub fn new() -> Self {
        let search_modes = vec![
            SearchMode {
                name: "Fuzzy".to_string(),
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
            searcher: None,
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
        }
    }

    pub async fn initialize(&mut self, directory: &std::path::Path, verbose: bool, respect_gitignore: bool) -> anyhow::Result<()> {
        self.status_message = "Indexing files...".to_string();
        
        let mut indexer = TreeSitterIndexer::with_options(verbose, respect_gitignore);
        indexer.initialize().await?;
        
        let patterns = vec!["**/*".to_string()];
        indexer.index_directory(directory, &patterns).await?;
        
        self.symbols = indexer.get_all_symbols();
        self.searcher = Some(FuzzySearcher::new(self.symbols.clone()));
        
        self.status_message = format!("Indexed {} symbols", self.symbols.len());
        
        // Show default results on startup
        self.show_default_results();
        
        Ok(())
    }

    fn detect_search_mode(&self, query: &str) -> SearchMode {
        if query.starts_with('#') {
            return self.search_modes[1].clone(); // Symbol
        } else if query.starts_with('>') {
            return self.search_modes[2].clone(); // File
        } else if query.starts_with('/') {
            return self.search_modes[3].clone(); // Regex
        }
        self.search_modes[0].clone() // Fuzzy (default)
    }

    fn extract_search_query(&self, query: &str) -> String {
        match &self.current_search_mode.prefix {
            prefix if !prefix.is_empty() && query.starts_with(prefix) => {
                query[prefix.len()..].to_string()
            }
            _ => query.to_string(),
        }
    }

    fn show_default_results(&mut self) {
        if self.symbols.is_empty() {
            return;
        }

        let default_symbols = self.sort_symbols_by_strategy(&self.default_strategy);
        self.current_results = default_symbols
            .into_iter()
            .take(100) // Limit to 100 results
            .map(|s| SearchResult {
                symbol: s,
                score: 1.0,
            })
            .collect();
        
        self.selected_index = 0;
    }

    fn sort_symbols_by_strategy(&self, strategy: &DefaultDisplayStrategy) -> Vec<CodeSymbol> {
        match strategy {
            DefaultDisplayStrategy::RecentlyModified => {
                self.sort_by_recent_modification()
            }
            DefaultDisplayStrategy::ProjectImportant => {
                self.sort_by_project_importance()
            }
            DefaultDisplayStrategy::SymbolBalance => {
                self.sort_by_symbol_balance()
            }
            DefaultDisplayStrategy::MostSymbols => {
                self.sort_by_most_symbols()
            }
            DefaultDisplayStrategy::Random => {
                self.sort_by_random()
            }
        }
    }

    fn sort_by_recent_modification(&self) -> Vec<CodeSymbol> {
        use std::collections::HashMap;
        
        // Group symbols by file and get file modification times
        let mut file_times: HashMap<std::path::PathBuf, std::time::SystemTime> = HashMap::new();
        
        for symbol in &self.symbols {
            if !file_times.contains_key(&symbol.file) {
                if let Ok(metadata) = std::fs::metadata(&symbol.file) {
                    if let Ok(modified) = metadata.modified() {
                        file_times.insert(symbol.file.clone(), modified);
                    }
                }
            }
        }
        
        // Sort symbols by file modification time (most recent first)
        let mut sorted_symbols = self.symbols.clone();
        sorted_symbols.sort_by(|a, b| {
            let time_a = file_times.get(&a.file).copied().unwrap_or(std::time::UNIX_EPOCH);
            let time_b = file_times.get(&b.file).copied().unwrap_or(std::time::UNIX_EPOCH);
            time_b.cmp(&time_a) // Reverse order for most recent first
        });
        
        sorted_symbols
    }

    fn sort_by_project_importance(&self) -> Vec<CodeSymbol> {
        // For now, return symbols as-is. This can be enhanced later.
        self.symbols.clone()
    }

    fn sort_by_symbol_balance(&self) -> Vec<CodeSymbol> {
        // For now, return symbols as-is. This can be enhanced later.
        self.symbols.clone()
    }

    fn sort_by_most_symbols(&self) -> Vec<CodeSymbol> {
        // For now, return symbols as-is. This can be enhanced later.
        self.symbols.clone()
    }

    fn sort_by_random(&self) -> Vec<CodeSymbol> {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        let mut symbols = self.symbols.clone();
        symbols.shuffle(&mut rng);
        symbols
    }

    pub fn perform_search(&mut self) {
        if let Some(ref searcher) = self.searcher {
            // Detect and update search mode
            self.current_search_mode = self.detect_search_mode(&self.query);
            
            if self.query.trim().is_empty() {
                // Show all symbols when query is empty (limit to 100)
                self.current_results = self.symbols
                    .iter()
                    .take(100)
                    .map(|s| SearchResult {
                        symbol: s.clone(),
                        score: 1.0,
                    })
                    .collect();
            } else {
                let mut clean_query = self.extract_search_query(&self.query);
                
                // Store whether the query was a directory search (ended with '/')
                let is_directory_search = clean_query.ends_with('/');
                
                // Remove trailing '/' for the actual search if it exists
                if is_directory_search {
                    clean_query = clean_query.trim_end_matches('/').to_string();
                }
                
                let search_options = match self.current_search_mode.name.as_str() {
                    "Symbol" => SearchOptions {
                        include_files: Some(false),
                        include_dirs: Some(false),
                        types: Some(vec![
                            SymbolType::Function,
                            SymbolType::Variable,
                            SymbolType::Class,
                            SymbolType::Interface,
                            SymbolType::Type,
                            SymbolType::Enum,
                            SymbolType::Constant,
                            SymbolType::Method,
                            SymbolType::Property,
                        ]),
                        ..Default::default()
                    },
                    "File" => {
                        // Check if the query ended with '/' for directory-only search
                        if is_directory_search {
                            SearchOptions {
                                types: Some(vec![SymbolType::Dirname]),
                                ..Default::default()
                            }
                        } else {
                            SearchOptions {
                                types: Some(vec![SymbolType::Filename, SymbolType::Dirname]),
                                ..Default::default()
                            }
                        }
                    },
                    _ => SearchOptions::default(), // Fuzzy or Regex
                };
                
                self.current_results = searcher.search(&clean_query, &search_options);
            }
            
            self.selected_index = 0;
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('?') => {
                self.show_help = !self.show_help;
            }
            KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected_index < self.current_results.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+P: Move up (previous)
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+N: Move down (next)
                if self.selected_index < self.current_results.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
            }
            KeyCode::Enter => {
                self.copy_current_result();
            }
            KeyCode::Char(c) => {
                self.query.push(c);
                self.perform_search();
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.perform_search();
            }
            _ => {}
        }
    }

    fn copy_current_result(&mut self) {
        if let Some(result) = self.current_results.get(self.selected_index) {
            let location = format!(
                "{}:{}:{}",
                result.symbol.file.display(),
                result.symbol.line,
                result.symbol.column
            );
            
            // Temporarily disable raw mode and restore normal terminal state for clipboard operation
            let clipboard_result = {
                // Disable raw mode temporarily
                if let Err(e) = disable_raw_mode() {
                    Some(format!("❌ Failed to disable raw mode: {}", e))
                } else {
                    // Perform clipboard operation in normal mode
                    let result = match Clipboard::new() {
                        Ok(mut clipboard) => {
                            match clipboard.set_text(&location) {
                                Ok(_) => None, // Success
                                Err(e) => Some(format!("❌ Failed to copy: {}", e)),
                            }
                        }
                        Err(e) => Some(format!("❌ Failed to access clipboard: {}", e)),
                    };
                    
                    // Re-enable raw mode
                    if let Err(e) = enable_raw_mode() {
                        Some(format!("❌ Failed to re-enable raw mode: {}", e))
                    } else {
                        result
                    }
                }
            };
            
            // Set status message based on result
            match clipboard_result {
                Some(error_msg) => {
                    self.status_message = error_msg;
                }
                None => {
                    self.status_message = format!("📋 Copied: {}", location);
                }
            }
            
            // Clear search box
            self.query.clear();
            self.perform_search();
        }
    }

    pub fn render(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search box
                Constraint::Min(1),    // Results
                Constraint::Length(3), // Status/help
            ])
            .split(f.size());

        self.render_search_box(f, chunks[0]);
        self.render_results(f, chunks[1]);
        self.render_status(f, chunks[2]);

        if self.show_help {
            self.render_help_popup(f);
        }
    }

    fn render_search_box(&self, f: &mut Frame, area: Rect) {
        let mode_info = format!("{} {} Search", self.current_search_mode.icon, self.current_search_mode.name);
        
        let search_text = Text::from(vec![
            Line::from(vec![
                Span::styled("Search: ", Style::default().fg(Color::Cyan)),
                Span::raw(&self.query),
                Span::styled("_", Style::default().fg(Color::Yellow)), // Cursor
            ]),
        ]);

        let search_box = Paragraph::new(search_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(mode_info)
                    .border_style(Style::default().fg(Color::Blue)),
            );

        f.render_widget(search_box, area);
    }

    fn render_results(&mut self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .current_results
            .iter()
            .enumerate()
            .map(|(i, result)| {
                let icon = match result.symbol.symbol_type {
                    SymbolType::Function => "🔧",
                    SymbolType::Variable => "📦",
                    SymbolType::Class => "🏗️",
                    SymbolType::Interface => "📐",
                    SymbolType::Type => "🔖",
                    SymbolType::Enum => "📝",
                    SymbolType::Constant => "🔒",
                    SymbolType::Method => "⚙️",
                    SymbolType::Property => "🔹",
                    SymbolType::Filename => "📄",
                    SymbolType::Dirname => "📁",
                };

                let line = Line::from(vec![
                    Span::raw(format!("{} ", icon)),
                    Span::styled(
                        &result.symbol.name,
                        if i == self.selected_index {
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                    Span::styled(
                        format!(" ({}:{})", result.symbol.file.display(), result.symbol.line),
                        Style::default().fg(Color::Gray),
                    ),
                ]);

                ListItem::new(line).style(
                    if i == self.selected_index {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    }
                )
            })
            .collect();

        let results_count = format!("Results: {}", self.current_results.len());
        let results_list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(results_count)
                    .border_style(Style::default().fg(Color::Green)),
            );

        let mut list_state = ListState::default();
        list_state.select(Some(self.selected_index));

        f.render_stateful_widget(results_list, area, &mut list_state);
    }

    fn render_status(&self, f: &mut Frame, area: Rect) {
        let status_text = Text::from(vec![
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Cyan)),
                Span::raw(&self.status_message),
            ]),
            Line::from(vec![
                Span::styled("Keys: ", Style::default().fg(Color::Yellow)),
                Span::raw("↑/↓/C-p/C-n Navigate • Enter Copy • ? Help • Esc/C-c Quit"),
            ]),
        ]);

        let status_box = Paragraph::new(status_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Status")
                    .border_style(Style::default().fg(Color::Magenta)),
            );

        f.render_widget(status_box, area);
    }

    fn render_help_popup(&self, f: &mut Frame) {
        let popup_area = centered_rect(60, 70, f.size());

        let help_text = Text::from(vec![
            Line::from("Symbol Fuzzy Search - Help"),
            Line::from(""),
            Line::from("Search Modes:"),
            Line::from("  🔍 Fuzzy - Default fuzzy search"),
            Line::from("  🏷️ #symbol - Search symbols only"),
            Line::from("  📁 >file - Search files/directories"),
            Line::from("  📁 >dir/ - Search directories only (with trailing /)"),
            Line::from("  🔧 /regex - Regular expression search"),
            Line::from(""),
            Line::from("Navigation:"),
            Line::from("  ↑/↓ or Ctrl+P/Ctrl+N - Move selection"),
            Line::from("  Enter - Copy location to clipboard"),
            Line::from("  Backspace - Delete character"),
            Line::from("  ? - Toggle this help"),
            Line::from("  Esc / Ctrl+C - Quit"),
            Line::from(""),
            Line::from("Note: By default, files ignored by .gitignore are excluded."),
            Line::from("      Use --include-ignored flag to search all files."),
            Line::from(""),
            Line::from("Press any key to close help"),
        ]);

        let help_popup = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Help")
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .style(Style::default().bg(Color::Black));

        f.render_widget(Clear, popup_area);
        f.render_widget(help_popup, popup_area);
    }
}

// Helper function to create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

pub async fn run_tui(directory: std::path::PathBuf, verbose: bool, respect_gitignore: bool) -> anyhow::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create and initialize app
    let mut app = TuiApp::new();
    app.initialize(&directory, verbose, respect_gitignore).await?;

    // Main loop
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut TuiApp,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| app.render(f))?;

        if app.should_quit {
            break;
        }

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key_event(key);
            }
        }
    }

    Ok(())
}