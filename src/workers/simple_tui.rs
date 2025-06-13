use crate::workers::{Worker, Message, WorkerMessage, SearchRouterMessage};
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
use std::io::{self};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::Duration;

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

/// シンプルなTUIワーカー
pub struct SimpleTuiWorker {
    worker_id: String,
    message_bus: Option<Arc<RwLock<crate::workers::MessageBus>>>,
    ui_update_sender: Option<mpsc::UnboundedSender<UiUpdate>>,
}

#[derive(Debug, Clone)]
enum UiUpdate {
    SearchClear,
    SearchMatch {
        filename: String,
        line: u32,
        column: u32,
        content: String,
    },
    IndexProgress {
        indexed_files: u32,
        total_files: u32,
        symbols: u32,
        elapsed: u64,
    },
}

impl SimpleTuiWorker {
    pub fn new(worker_id: String) -> Self {
        Self {
            worker_id,
            message_bus: None,
            ui_update_sender: None,
        }
    }

    pub fn set_message_bus(&mut self, message_bus: Arc<RwLock<crate::workers::MessageBus>>) {
        self.message_bus = Some(message_bus);
    }



    fn handle_search_message(&self, message: SearchRouterMessage) {
        if let Some(sender) = &self.ui_update_sender {
            let update = match message {
                SearchRouterMessage::SearchClear => UiUpdate::SearchClear,
                SearchRouterMessage::SearchMatch { filename, line, column, content } => {
                    UiUpdate::SearchMatch { filename, line, column, content }
                }
                SearchRouterMessage::IndexProgress { indexed_files, total_files, symbols, elapsed } => {
                    UiUpdate::IndexProgress { indexed_files, total_files, symbols, elapsed }
                }
                SearchRouterMessage::IndexUpdate { .. } => return, // 無視
            };

            let _ = sender.send(update);
        }
    }
}

#[async_trait]
impl Worker for SimpleTuiWorker {
    fn worker_id(&self) -> &str {
        &self.worker_id
    }

    async fn initialize(&mut self) -> Result<(), crate::workers::worker::WorkerError> {
        // UIループを非同期で開始
        let message_bus = self.message_bus.clone();
        let (ui_sender, ui_receiver) = mpsc::unbounded_channel::<UiUpdate>();
        self.ui_update_sender = Some(ui_sender);

        tokio::spawn(async move {
            if let Err(e) = run_ui_main_loop(message_bus, ui_receiver).await {
                eprintln!("UI loop error: {}", e);
            }
            std::process::exit(0);
        });

        Ok(())
    }

    async fn handle_message(&mut self, message: Message) -> Result<(), crate::workers::worker::WorkerError> {
        if let Ok(worker_msg) = WorkerMessage::try_from(message) {
            match worker_msg {
                WorkerMessage::SearchRouter(search_msg) => {
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
        // UIループがターミナル復元を行うため、特別な処理は不要
        Ok(())
    }
}

/// UIメインループを実行する関数
async fn run_ui_main_loop(
    message_bus: Option<Arc<RwLock<crate::workers::MessageBus>>>,
    mut ui_receiver: mpsc::UnboundedReceiver<UiUpdate>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // ターミナル初期化
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // UI状態
    let mut current_query = String::new();
    let mut search_results: Vec<SearchMatch> = Vec::new();
    let mut selected_index = 0;
    let mut index_progress: Option<IndexProgressInfo> = None;

    // メインループ
    loop {
        // UI更新の処理
        while let Ok(update) = ui_receiver.try_recv() {
            match update {
                UiUpdate::SearchClear => {
                    search_results.clear();
                    selected_index = 0;
                }
                UiUpdate::SearchMatch { filename, line, column, content } => {
                    search_results.push(SearchMatch { filename, line, column, content });
                }
                UiUpdate::IndexProgress { indexed_files, total_files, symbols, elapsed } => {
                    index_progress = Some(IndexProgressInfo { indexed_files, total_files, symbols, elapsed });
                }
            }
        }

        // 画面描画
        terminal.draw(|f| {
            draw_ui_static(f, &current_query, &search_results, selected_index, &index_progress);
        })?;

        // イベント処理（非ブロッキング）
        if event::poll(Duration::from_millis(16))? {
            let event = event::read()?;
            if handle_input_event_static(
                event, 
                &mut current_query, 
                &search_results, 
                &mut selected_index,
                &message_bus,
            ).await? {
                break; // quit
            }
        }

        tokio::time::sleep(Duration::from_millis(16)).await;
    }

    // ターミナル復元
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

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

    // 検索結果（スクロール処理付き）
    let visible_height = chunks[1].height.saturating_sub(2) as usize; // ボーダーを除く
    
    // スクロールオフセットを計算（選択されたアイテムが見えるように）
    let scroll_offset = if selected_index >= visible_height {
        selected_index.saturating_sub(visible_height - 1)
    } else {
        0
    };

    let items: Vec<ListItem> = search_results
        .iter()
        .enumerate()
        .skip(scroll_offset)  // スクロールオフセットから開始
        .take(visible_height) // 表示可能な行数まで
        .map(|(original_index, result)| {
            let is_selected = original_index == selected_index;
            let style = if is_selected {
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

async fn handle_input_event_static(
    event: Event,
    current_query: &mut String,
    search_results: &[SearchMatch],
    selected_index: &mut usize,
    message_bus: &Option<Arc<RwLock<crate::workers::MessageBus>>>,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    match event {
        Event::Key(KeyEvent { code, modifiers, .. }) => {
            match code {
                KeyCode::Esc => {
                    return Ok(true); // quit
                }
                KeyCode::Enter => {
                    copy_selected_result_static(search_results, *selected_index).await?;
                }
                KeyCode::Up => {
                    if *selected_index > 0 {
                        *selected_index -= 1;
                    }
                }
                KeyCode::Down => {
                    if *selected_index < search_results.len().saturating_sub(1) {
                        *selected_index += 1;
                    }
                }
                KeyCode::Char(c) => {
                    if modifiers.contains(KeyModifiers::CONTROL) {
                        match c {
                            'c' => return Ok(true), // quit on Ctrl+C
                            'u' => {
                                current_query.clear();
                                send_query_static(current_query, message_bus).await?;
                            }
                            _ => {}
                        }
                    } else {
                        current_query.push(c);
                        send_query_static(current_query, message_bus).await?;
                    }
                }
                KeyCode::Backspace => {
                    current_query.pop();
                    send_query_static(current_query, message_bus).await?;
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(false)
}

async fn send_query_static(
    query: &str,
    message_bus: &Option<Arc<RwLock<crate::workers::MessageBus>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(bus) = message_bus {
        let message = WorkerMessage::tui_query(query.to_string());
        let msg: Message = message.into();
        
        let bus_guard = bus.read().await;
        bus_guard.send_to("search_router", msg).map_err(|e| format!("Failed to send query: {}", e))?;
    }
    Ok(())
}

async fn copy_selected_result_static(
    search_results: &[SearchMatch],
    selected_index: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(result) = search_results.get(selected_index) {
        // クリップボードにコピー
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let text = format!("{}:{}:{}", result.filename, result.line, result.column);
            let _ = clipboard.set_text(text);
        }
    }
    Ok(())
}