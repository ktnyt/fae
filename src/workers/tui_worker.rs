use crate::workers::{Worker, Message, WorkerMessage, SearchHandlerMessage};
use async_trait::async_trait;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io::{self, Stdout};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;

/// UI状態を管理する構造体
struct TuiState {
    current_query: String,
    search_results: Vec<SearchMatch>,
    selected_index: usize,
    index_progress: Option<IndexProgressInfo>,
    message_bus: Option<Arc<RwLock<crate::workers::MessageBus>>>,
}

impl TuiState {
    fn draw_ui(&self, frame: &mut Frame) {
        TuiWorker::draw_ui_static(
            frame, 
            &self.current_query, 
            &self.search_results, 
            self.selected_index, 
            &self.index_progress
        );
    }

    async fn handle_input_event(&mut self, event: Event) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        match event {
            Event::Key(KeyEvent { code, modifiers, .. }) => {
                match code {
                    KeyCode::Esc => {
                        return Ok(true); // quit
                    }
                    KeyCode::Enter => {
                        self.copy_selected_result().await?;
                    }
                    KeyCode::Up => {
                        if self.selected_index > 0 {
                            self.selected_index -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if self.selected_index < self.search_results.len().saturating_sub(1) {
                            self.selected_index += 1;
                        }
                    }
                    KeyCode::Char(c) => {
                        if modifiers.contains(KeyModifiers::CONTROL) {
                            match c {
                                'c' => return Ok(true), // quit on Ctrl+C
                                'u' => {
                                    self.current_query.clear();
                                    self.send_query().await?;
                                }
                                _ => {}
                            }
                        } else {
                            self.current_query.push(c);
                            self.send_query().await?;
                        }
                    }
                    KeyCode::Backspace => {
                        self.current_query.pop();
                        self.send_query().await?;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(false)
    }

    async fn send_query(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(bus) = &self.message_bus {
            let message = WorkerMessage::tui_query(self.current_query.clone());
            let msg: Message = message.into();
            
            let bus_guard = bus.read().await;
            bus_guard.send_to("search_handler", msg).map_err(|e| format!("Failed to send query: {}", e))?;
        }
        Ok(())
    }

    async fn copy_selected_result(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(result) = self.search_results.get(self.selected_index) {
            // クリップボードにコピー
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let text = format!("{}:{}:{}", result.filename, result.line, result.column);
                let _ = clipboard.set_text(text);
            }
        }
        Ok(())
    }
}

/// ターミナル復元のためのヘルパー関数
async fn restore_terminal_static(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

pub struct TuiWorker {
    worker_id: String,
    terminal: Option<Terminal<CrosstermBackend<Stdout>>>,
    message_bus: Option<Arc<RwLock<crate::workers::MessageBus>>>,
    
    // UI状態
    current_query: String,
    search_results: Vec<SearchMatch>,
    selected_index: usize,
    
    // インデックス状態
    index_progress: Option<IndexProgressInfo>,
}

#[derive(Debug, Clone)]
struct SearchMatch {
    filename: String,
    line: u32,
    column: u32,
    content: String,
}

#[derive(Debug, Clone)]
struct IndexProgressInfo {
    indexed_files: u32,
    total_files: u32,
    symbols: u32,
    elapsed: u64,
}

impl TuiWorker {
    pub fn new(worker_id: String) -> Self {
        Self {
            worker_id,
            terminal: None,
            message_bus: None,
            current_query: String::new(),
            search_results: Vec::new(),
            selected_index: 0,
            index_progress: None,
        }
    }

    pub fn set_message_bus(&mut self, message_bus: Arc<RwLock<crate::workers::MessageBus>>) {
        self.message_bus = Some(message_bus);
    }

    async fn setup_terminal(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        self.terminal = Some(terminal);
        Ok(())
    }

    async fn restore_terminal(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(mut terminal) = self.terminal.take() {
            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;
        }
        Ok(())
    }


    async fn handle_input_event(&mut self, event: Event) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        match event {
            Event::Key(KeyEvent { code, modifiers, .. }) => {
                match code {
                    KeyCode::Esc => {
                        return Ok(true); // quit
                    }
                    KeyCode::Enter => {
                        self.copy_selected_result().await?;
                    }
                    KeyCode::Up => {
                        if self.selected_index > 0 {
                            self.selected_index -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if self.selected_index < self.search_results.len().saturating_sub(1) {
                            self.selected_index += 1;
                        }
                    }
                    KeyCode::Char(c) => {
                        if modifiers.contains(KeyModifiers::CONTROL) {
                            match c {
                                'c' => return Ok(true), // quit on Ctrl+C
                                'u' => {
                                    self.current_query.clear();
                                    self.send_query().await?;
                                }
                                _ => {}
                            }
                        } else {
                            self.current_query.push(c);
                            self.send_query().await?;
                        }
                    }
                    KeyCode::Backspace => {
                        self.current_query.pop();
                        self.send_query().await?;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(false)
    }

    async fn send_query(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(bus) = &self.message_bus {
            let message = WorkerMessage::tui_query(self.current_query.clone());
            let msg: Message = message.into();
            
            let bus_guard = bus.read().await;
            bus_guard.send_to("search_handler", msg).map_err(|e| format!("Failed to send query: {}", e))?;
        }
        Ok(())
    }

    async fn copy_selected_result(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(result) = self.search_results.get(self.selected_index) {
            // クリップボードにコピー
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let text = format!("{}:{}:{}", result.filename, result.line, result.column);
                let _ = clipboard.set_text(text);
            }
        }
        Ok(())
    }

    fn handle_search_message(&mut self, message: SearchHandlerMessage) {
        match message {
            SearchHandlerMessage::SearchClear => {
                self.search_results.clear();
                self.selected_index = 0;
            }
            SearchHandlerMessage::SearchMatch { filename, line, column, content } => {
                self.search_results.push(SearchMatch {
                    filename,
                    line,
                    column,
                    content,
                });
            }
            SearchHandlerMessage::IndexProgress { indexed_files, total_files, symbols, elapsed } => {
                self.index_progress = Some(IndexProgressInfo {
                    indexed_files,
                    total_files,
                    symbols,
                    elapsed,
                });
            }
            SearchHandlerMessage::IndexUpdate { filename: _, symbols: _, elapsed: _ } => {
                // IndexUpdateは現在の実装では特に処理しない
            }
        }
    }

    async fn run_ui_loop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        loop {
            // 画面描画
            if let Some(terminal) = &mut self.terminal {
                let current_query = self.current_query.clone();
                let search_results = self.search_results.clone();
                let selected_index = self.selected_index;
                let index_progress = self.index_progress.clone();

                terminal.draw(|f| {
                    Self::draw_ui_static(f, &current_query, &search_results, selected_index, &index_progress);
                })?;
            }

            // イベントチェック（非ブロッキング）
            if event::poll(Duration::from_millis(16))? {
                let event = event::read()?;
                if self.handle_input_event(event).await? {
                    break; // quit
                }
            }

            // 少し待機してCPU使用率を下げる
            tokio::time::sleep(Duration::from_millis(16)).await;
        }
        Ok(())
    }

    fn draw_ui_static(
        frame: &mut Frame,
        current_query: &str,
        search_results: &[SearchMatch],
        selected_index: usize,
        index_progress: &Option<IndexProgressInfo>,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // 検索ボックス
                Constraint::Min(0),     // 結果リスト
                Constraint::Length(3),  // ステータス行
            ])
            .split(frame.size());

        // 検索ボックス
        let query_paragraph = Paragraph::new(current_query)
            .block(Block::default().borders(Borders::ALL).title("Search Query"));
        frame.render_widget(query_paragraph, chunks[0]);

        // 検索結果
        let items: Vec<ListItem> = search_results
            .iter()
            .enumerate()
            .map(|(i, result)| {
                let style = if i == selected_index {
                    Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let line = Line::from(vec![
                    Span::styled(
                        format!("{}:{}:{}", result.filename, result.line, result.column),
                        Style::default().fg(Color::Green),
                    ),
                    Span::raw(" "),
                    Span::raw(&result.content),
                ]);

                ListItem::new(line).style(style)
            })
            .collect();

        let results_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Search Results"));
        frame.render_widget(results_list, chunks[1]);

        // ステータス行
        let status_text = if let Some(progress) = index_progress {
            format!(
                "Indexed: {}/{} files, {} symbols, {}ms elapsed",
                progress.indexed_files, progress.total_files, progress.symbols, progress.elapsed
            )
        } else {
            format!("Results: {} | Use ↑↓ to navigate, Enter to copy, Esc to quit", search_results.len())
        };

        let status_paragraph = Paragraph::new(status_text)
            .block(Block::default().borders(Borders::ALL).title("Status"));
        frame.render_widget(status_paragraph, chunks[2]);
    }
}

#[async_trait]
impl Worker for TuiWorker {
    fn worker_id(&self) -> &str {
        &self.worker_id
    }

    async fn initialize(&mut self) -> Result<(), crate::workers::worker::WorkerError> {
        // ターミナル初期化
        self.setup_terminal().await
            .map_err(|e| crate::workers::worker::WorkerError::InitializationFailed(e.to_string()))?;
        
        // UIループを開始（ワーカーのメッセージ処理と並行実行）
        let mut ui_state = TuiState {
            current_query: self.current_query.clone(),
            search_results: self.search_results.clone(),
            selected_index: self.selected_index,
            index_progress: self.index_progress.clone(),
            message_bus: self.message_bus.clone(),
        };

        // UIループをバックグラウンドタスクとして実行
        if let Some(mut terminal) = self.terminal.take() {
            tokio::spawn(async move {
                loop {
                    // 描画
                    if let Err(e) = terminal.draw(|f| {
                        ui_state.draw_ui(f);
                    }) {
                        eprintln!("Draw error: {}", e);
                        break;
                    }

                    // イベント処理
                    if event::poll(Duration::from_millis(16)).unwrap_or(false) {
                        if let Ok(event) = event::read() {
                            if ui_state.handle_input_event(event).await.unwrap_or(false) {
                                break; // quit
                            }
                        }
                    }

                    tokio::time::sleep(Duration::from_millis(16)).await;
                }

                // ターミナル復元
                let _ = restore_terminal_static(&mut terminal).await;
                std::process::exit(0);
            });
        }

        Ok(())
    }

    async fn handle_message(&mut self, message: Message) -> Result<(), crate::workers::worker::WorkerError> {
        if let Ok(worker_msg) = WorkerMessage::try_from(message) {
            match worker_msg {
                WorkerMessage::SearchHandler(search_msg) => {
                    self.handle_search_message(search_msg);
                }
                _ => {
                    // 他のメッセージタイプは処理しない
                }
            }
        }
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), crate::workers::worker::WorkerError> {
        self.restore_terminal().await
            .map_err(|e| crate::workers::worker::WorkerError::CleanupFailed(e.to_string()))?;
        Ok(())
    }
}