use crate::actors::{messages::FaeMessage, types::{SearchParams, SearchResult, SearchMode as ActorSearchMode}};
use crate::unified_search::UnifiedSearchSystem;
use crate::core::Message;
use arboard::Clipboard;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
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
use std::{
    env,
    io::{stdout, Stdout},
    process::Command,
    time::{Duration, Instant},
};
use tokio::time::sleep;

pub struct TuiApp {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    search_input: String,
    search_results: Vec<SearchResult>,
    list_state: ListState,
    search_mode: SearchMode,
    last_search_time: Instant,
    indexing_progress: Option<IndexingProgress>,
    show_toast: bool,
    toast_end_time: Option<Instant>,
    debounce_timer: Option<Instant>,
    result_receiver: tokio::sync::mpsc::UnboundedReceiver<Message<FaeMessage>>,
    control_sender: tokio::sync::mpsc::UnboundedSender<Message<FaeMessage>>,
    _search_system: UnifiedSearchSystem, // Keep reference for lifecycle management
}

#[derive(Clone, Debug)]
pub enum SearchMode {
    Content,
    Symbol,
    File,
    Regex,
}

impl SearchMode {
    fn prefix(&self) -> &'static str {
        match self {
            SearchMode::Content => "",
            SearchMode::Symbol => "#",
            SearchMode::File => ">",
            SearchMode::Regex => "/",
        }
    }

    fn display_name(&self) -> &'static str {
        match self {
            SearchMode::Content => "Content",
            SearchMode::Symbol => "Symbol",
            SearchMode::File => "File",
            SearchMode::Regex => "Regex",
        }
    }

    fn next(&self) -> Self {
        match self {
            SearchMode::Content => SearchMode::Symbol,
            SearchMode::Symbol => SearchMode::File,
            SearchMode::File => SearchMode::Regex,
            SearchMode::Regex => SearchMode::Content,
        }
    }

    fn prev(&self) -> Self {
        match self {
            SearchMode::Content => SearchMode::Regex,
            SearchMode::Symbol => SearchMode::Content,
            SearchMode::File => SearchMode::Symbol,
            SearchMode::Regex => SearchMode::File,
        }
    }
}

#[derive(Clone, Debug)]
struct IndexingProgress {
    files_processed: u32,
    total_files: u32,
    symbols_found: u32,
    is_complete: bool,
}

impl TuiApp {
    pub async fn new(search_path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        // Create channels for TUI communication
        let (control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (result_tx, result_rx) = tokio::sync::mpsc::unbounded_channel();
        
        // Create search system with watch_files=true for TUI
        let search_system = UnifiedSearchSystem::new(search_path, true, result_tx, control_receiver).await?;

        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Ok(TuiApp {
            terminal,
            search_input: String::new(),
            search_results: Vec::new(),
            list_state,
            search_mode: SearchMode::Content,
            last_search_time: Instant::now(),
            indexing_progress: None,
            show_toast: false,
            toast_end_time: None,
            debounce_timer: None,
            result_receiver: result_rx,
            control_sender,
            _search_system: search_system,
        })
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Start indexing
        self.start_indexing().await;

        loop {
            // Draw UI
            self.draw_ui()?;

            // Check for search results
            if let Ok(message) = self.result_receiver.try_recv() {
                match &message.payload {
                    FaeMessage::PushSearchResult(result) => {
                        self.search_results.push(result.clone());
                        if self.list_state.selected().is_none() {
                            self.list_state.select(Some(0));
                        }
                    }
                    FaeMessage::ClearResults => {
                        self.search_results.clear();
                        self.list_state.select(Some(0));
                    }
                    _ => {}
                }
            }

            // Handle events with timeout for responsive UI
            if event::poll(Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_key_event(key).await? {
                        break;
                    }
                }
            }

            // Handle debounced search
            self.handle_debounced_search().await;

            // Update toast display
            self.update_toast_display();

            // Small delay to prevent excessive CPU usage
            sleep(Duration::from_millis(16)).await;
        }

        // Cleanup
        self.cleanup()?;
        Ok(())
    }

    async fn start_indexing(&mut self) {
        self.indexing_progress = Some(IndexingProgress {
            files_processed: 0,
            total_files: 0,
            symbols_found: 0,
            is_complete: false,
        });
        self.show_toast = true;
        
        // TODO: Implement actual progress tracking
        // For now, simulate completion after a short delay
        tokio::spawn(async {
            sleep(Duration::from_secs(2)).await;
            // In real implementation, this would be updated by messages from actors
        });
    }

    async fn handle_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(true),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(true),
            
            // Navigation
            KeyCode::Down | KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.next_result();
            }
            KeyCode::Up | KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.prev_result();
            }
            
            // Mode switching
            KeyCode::Tab => {
                self.search_mode = self.search_mode.next();
                self.trigger_search().await;
            }
            KeyCode::BackTab => {
                self.search_mode = self.search_mode.prev();
                self.trigger_search().await;
            }
            
            // Open selected result
            KeyCode::Enter => {
                self.open_selected_result().await?;
            }
            
            // Text input
            KeyCode::Char(c) => {
                self.search_input.push(c);
                self.debounce_timer = Some(Instant::now());
            }
            KeyCode::Backspace => {
                self.search_input.pop();
                self.debounce_timer = Some(Instant::now());
            }
            
            _ => {}
        }
        Ok(false)
    }

    async fn handle_debounced_search(&mut self) {
        if let Some(timer) = self.debounce_timer {
            if timer.elapsed() >= Duration::from_millis(300) {
                self.trigger_search().await;
                self.debounce_timer = None;
            }
        }
    }

    async fn trigger_search(&mut self) {
        if self.search_input.is_empty() {
            self.search_results.clear();
            self.list_state.select(Some(0));
            return;
        }

        let query = format!("{}{}", self.search_mode.prefix(), self.search_input);
        let _search_params = SearchParams {
            query,
            mode: match self.search_mode {
                SearchMode::Content => ActorSearchMode::Literal,
                SearchMode::Symbol => ActorSearchMode::Symbol,
                SearchMode::File => ActorSearchMode::Filepath,
                SearchMode::Regex => ActorSearchMode::Regexp,
            },
        };

        // Send search request to unified search system
        let search_message = Message::new("updateSearchParams", FaeMessage::UpdateSearchParams(_search_params));
        if let Err(e) = self.control_sender.send(search_message) {
            log::error!("Failed to send search message: {}", e);
        }

        // Clear current results - new results will come via result_receiver
        self.search_results.clear();
        self.list_state.select(Some(0));
        self.last_search_time = Instant::now();
    }

    fn next_result(&mut self) {
        if !self.search_results.is_empty() {
            let i = match self.list_state.selected() {
                Some(i) => {
                    if i >= self.search_results.len() - 1 {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            self.list_state.select(Some(i));
        }
    }

    fn prev_result(&mut self) {
        if !self.search_results.is_empty() {
            let i = match self.list_state.selected() {
                Some(i) => {
                    if i == 0 {
                        self.search_results.len() - 1
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            self.list_state.select(Some(i));
        }
    }

    async fn open_selected_result(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(selected) = self.list_state.selected() {
            if let Some(result) = self.search_results.get(selected) {
                let fae_open = env::var("FAE_OPEN").unwrap_or_else(|_| "clipboard".to_string());
                
                if fae_open == "clipboard" {
                    // Copy to clipboard
                    let mut clipboard = Clipboard::new()?;
                    let content = format!("{}:{}", result.filename, result.line);
                    clipboard.set_text(content)?;
                } else {
                    // Execute custom command
                    let file_location = format!("{}:{}", result.filename, result.line);
                    Command::new("sh")
                        .arg("-c")
                        .arg(&fae_open.replace("{}", &file_location))
                        .spawn()?;
                }
            }
        }
        Ok(())
    }

    fn update_toast_display(&mut self) {
        if let Some(progress) = &mut self.indexing_progress {
            if progress.is_complete {
                if self.toast_end_time.is_none() {
                    self.toast_end_time = Some(Instant::now() + Duration::from_secs(3));
                } else if let Some(end_time) = self.toast_end_time {
                    if Instant::now() >= end_time {
                        self.show_toast = false;
                        self.indexing_progress = None;
                        self.toast_end_time = None;
                    }
                }
            }
        }
    }

    fn draw_ui(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let input = self.search_input.clone();
        let results = self.search_results.clone();
        let mode = self.search_mode.clone();
        let progress = self.indexing_progress.clone();
        let show_toast = self.show_toast;
        let mut list_state = self.list_state.clone();
        
        self.terminal.draw(|f| {
            render_ui_static(f, &input, &results, &mode, &progress, show_toast, &mut list_state);
        })?;
        
        self.list_state = list_state;
        Ok(())
    }
}

fn render_ui_static(
    f: &mut Frame,
    input: &str,
    results: &[SearchResult],
    mode: &SearchMode,
    progress: &Option<IndexingProgress>,
    show_toast: bool,
    list_state: &mut ListState,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search box
            Constraint::Min(1),    // Results
            Constraint::Length(3), // Status bar
        ])
        .split(f.size());

    // Search box
    render_search_box_static(f, chunks[0], input, mode);

    // Results
    render_results_static(f, chunks[1], results, list_state);

    // Status bar
    render_status_bar_static(f, chunks[2]);

    // Toast (if visible)
    if show_toast {
        render_toast_static(f, progress);
    }
}

fn render_search_box_static(f: &mut Frame, area: Rect, input: &str, mode: &SearchMode) {
    let mode_indicator = format!("[{}] ", mode.display_name());
    let full_input = format!("{}{}", mode_indicator, input);
    
    let paragraph = Paragraph::new(full_input)
        .block(Block::default().borders(Borders::ALL).title("Search"))
        .style(Style::default().fg(Color::White));
    
    f.render_widget(paragraph, area);
}

fn render_results_static(f: &mut Frame, area: Rect, results: &[SearchResult], list_state: &mut ListState) {
    let items: Vec<ListItem> = results
        .iter()
        .map(|result| {
            let line = format!(
                "{}:{} - {}",
                result.filename,
                result.line,
                result.content.trim()
            );
            ListItem::new(line)
        })
        .collect();

    let results_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Results"))
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(results_list, area, list_state);
}

fn render_status_bar_static(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Span::raw("↑↓/Ctrl+P/N: Navigate | "),
        Span::raw("Tab/Shift+Tab: Mode | "),
        Span::raw("Enter: Open | "),
        Span::raw("Esc/Ctrl+C: Quit"),
    ];
    
    let paragraph = Paragraph::new(Line::from(help_text))
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .style(Style::default().fg(Color::Gray));
    
    f.render_widget(paragraph, area);
}

fn render_toast_static(f: &mut Frame, progress: &Option<IndexingProgress>) {
    if let Some(progress) = progress {
        let area = centered_rect(30, 20, f.size());
        
        let toast_text = if progress.is_complete {
            format!(
                "Indexing Complete!\nFiles: {}\nSymbols: {}",
                progress.total_files, progress.symbols_found
            )
        } else {
            format!(
                "Indexing... {}/{}",
                progress.files_processed, progress.total_files
            )
        };

        let toast = Paragraph::new(toast_text)
            .block(Block::default().borders(Borders::ALL).title("Progress"))
            .style(Style::default().fg(Color::Green));

        f.render_widget(Clear, area);
        f.render_widget(toast, area);
    }
}

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

impl TuiApp {
    fn cleanup(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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