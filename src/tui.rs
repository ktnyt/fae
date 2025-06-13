//! TUI: 非同期Iterator + メッセージエンジンアーキテクチャ
//! 
//! - ユーザー入力を非同期Streamで処理
//! - 検索バックエンドからの応答を非同期Streamで処理  
//! - メッセージエンジンが複数のStreamを統合してUI更新・検索リクエストを処理

use crate::types::{SearchResult, SearchMode};
use crate::cli::SearchRunner;
use anyhow::Result;
use crossterm::event::{Event as CrosstermEvent, KeyEvent, MouseEvent};
use futures_util::{Stream, StreamExt};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::UnboundedReceiverStream;

/// クエリからモードを検出（プレフィックスベース）
fn detect_mode(query: &str) -> (SearchMode, String) {
    if query.starts_with('#') {
        (SearchMode::Symbol, query[1..].to_string())
    } else if query.starts_with('>') {
        (SearchMode::File, query[1..].to_string())
    } else if query.starts_with('/') {
        (SearchMode::Regex, query[1..].to_string())
    } else {
        (SearchMode::Content, query.to_string())
    }
}

/// TUIで処理するイベントの種類
#[derive(Debug)]
pub enum TuiEvent {
    /// ユーザー入力（キーボード・マウス）
    Input(InputEvent),
    
    /// 検索関連イベント
    Search(SearchEvent),
    
    /// UI再描画タイマー
    Tick,
    
    /// アプリケーション終了
    Quit,
}

/// ユーザー入力イベント
#[derive(Debug)]
pub enum InputEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
}

/// 検索関連イベント
#[derive(Debug)]
pub enum SearchEvent {
    /// 検索開始
    Started { query: String, mode: SearchMode },
    
    /// 検索結果（部分）
    Results(Vec<SearchResult>),
    
    /// 検索完了
    Completed,
    
    /// 検索エラー
    Error(String),
}

/// 検索リクエスト
#[derive(Debug)]
pub struct SearchQuery {
    pub query: String,
    pub mode: SearchMode,
    pub project_root: PathBuf,
    pub response_tx: oneshot::Sender<Result<Vec<SearchResult>>>,
}

/// TUIアプリケーションの状態
#[derive(Debug)]
pub struct TuiState {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub selected_index: usize,
    pub search_mode: SearchMode,
    pub loading: bool,
    pub error_message: Option<String>,
    pub project_root: PathBuf,
}

impl TuiState {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected_index: 0,
            search_mode: SearchMode::Content,
            loading: false,
            error_message: None,
            project_root,
        }
    }
    
    /// クエリを更新してモードを自動検出
    pub fn update_query(&mut self, new_query: String) {
        self.query = new_query;
        self.detect_and_update_mode();
    }
    
    /// クエリからモードを検出して更新
    fn detect_and_update_mode(&mut self) {
        let (mode, _clean_query) = detect_mode(&self.query);
        self.search_mode = mode;
    }
    
    /// クリーンなクエリ（プレフィックスなし）を取得
    pub fn clean_query(&self) -> String {
        let (_mode, clean_query) = detect_mode(&self.query);
        clean_query
    }
    
    /// 次のアイテムを選択
    pub fn select_next(&mut self) {
        if !self.results.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.results.len();
        }
    }
    
    /// 前のアイテムを選択
    pub fn select_previous(&mut self) {
        if !self.results.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.results.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }
    
    /// 選択されたアイテムを取得
    pub fn selected_result(&self) -> Option<&SearchResult> {
        self.results.get(self.selected_index)
    }
}

/// ユーザー入力を非同期Streamに変換
pub fn create_input_stream() -> impl Stream<Item = InputEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    
    // バックグラウンドタスクでcrossterm eventsを監視
    tokio::spawn(async move {
        loop {
            // 非ブロッキングでイベントをポーリング
            if crossterm::event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(event) = crossterm::event::read() {
                    let input_event = match event {
                        CrosstermEvent::Key(key) => InputEvent::Key(key),
                        CrosstermEvent::Mouse(mouse) => InputEvent::Mouse(mouse),
                        CrosstermEvent::Resize(w, h) => InputEvent::Resize(w, h),
                        _ => continue,
                    };
                    
                    if tx.send(input_event).is_err() {
                        break;
                    }
                } else {
                    break; // エラーが発生した場合は終了
                }
            }
            
            // 少し待機してCPU使用率を抑える
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
    
    UnboundedReceiverStream::new(rx)
}

/// 検索結果を非同期Streamに変換
pub fn create_search_stream(
    search_runner: SearchRunner,
    mut query_rx: mpsc::UnboundedReceiver<SearchQuery>,
) -> impl Stream<Item = SearchEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    
    // バックグラウンドタスクで検索リクエストを処理
    tokio::spawn(async move {
        while let Some(search_query) = query_rx.recv().await {
            let SearchQuery { query, mode, project_root, response_tx } = search_query;
            
            // 検索開始を通知
            let _ = tx.send(SearchEvent::Started { 
                query: query.clone(), 
                mode: mode.clone()
            });
            
            // 検索実行（新しい非同期メソッドが必要）
            let result = execute_search(&search_runner, &query, mode, &project_root).await;
            
            match result {
                Ok(results) => {
                    // 結果をストリームに送信
                    let _ = tx.send(SearchEvent::Results(results.clone()));
                    let _ = tx.send(SearchEvent::Completed);
                    
                    // 応答も送信
                    let _ = response_tx.send(Ok(results));
                }
                Err(err) => {
                    let error_msg = err.to_string();
                    let _ = tx.send(SearchEvent::Error(error_msg.clone()));
                    let _ = response_tx.send(Err(err));
                }
            }
        }
    });
    
    UnboundedReceiverStream::new(rx)
}

/// 検索を非同期で実行
async fn execute_search(
    search_runner: &SearchRunner,
    query: &str,
    mode: SearchMode,
    project_root: &PathBuf,
) -> Result<Vec<SearchResult>> {
    use crate::cli::strategies::{ContentStrategy, SymbolStrategy, FileStrategy, RegexStrategy};
    
    let search_runner = search_runner.clone();
    let query = query.to_string();
    let mode = mode;
    let _project_root = project_root.clone();
    
    tokio::task::spawn_blocking(move || {
        // 検索モードに応じて適切な戦略を選択
        match mode {
            SearchMode::Content => {
                let strategy = ContentStrategy;
                search_runner.collect_results_with_strategy(&strategy, &query)
            }
            SearchMode::Symbol => {
                let strategy = SymbolStrategy::new();
                search_runner.collect_results_with_strategy(&strategy, &query)
            }
            SearchMode::File => {
                let strategy = FileStrategy;
                search_runner.collect_results_with_strategy(&strategy, &query)
            }
            SearchMode::Regex => {
                let strategy = RegexStrategy;
                search_runner.collect_results_with_strategy(&strategy, &query)
            }
        }
    }).await.map_err(|e| anyhow::anyhow!("Search task failed: {}", e))?
}

/// メッセージ処理エンジン
pub struct TuiEngine {
    state: TuiState,
    input_stream: std::pin::Pin<Box<dyn Stream<Item = InputEvent> + Send>>,
    search_stream: std::pin::Pin<Box<dyn Stream<Item = SearchEvent> + Send>>,
    search_tx: mpsc::UnboundedSender<SearchQuery>,
    tick_interval: tokio::time::Interval,
    terminal: ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
}

impl TuiEngine {
    /// 新しいTuiEngineを作成
    pub fn new(project_root: PathBuf, search_runner: SearchRunner) -> Result<Self> {
        use crossterm::{
            execute,
            terminal::{enable_raw_mode, EnterAlternateScreen},
        };
        use ratatui::{backend::CrosstermBackend, Terminal};
        
        let state = TuiState::new(project_root);
        
        // ユーザー入力ストリーム
        let input_stream = Box::pin(create_input_stream());
        
        // 検索ストリーム
        let (search_tx, search_rx) = mpsc::unbounded_channel();
        let search_stream = Box::pin(create_search_stream(search_runner, search_rx));
        
        // UIティックタイマー（60fps）
        let tick_interval = tokio::time::interval(Duration::from_millis(16));
        
        // Terminal初期化（一度だけ）
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        enable_raw_mode()?;
        
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        
        Ok(Self {
            state,
            input_stream,
            search_stream,
            search_tx,
            tick_interval,
            terminal,
        })
    }
    
    /// メインイベントループ
    pub async fn run(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                biased;
                
                // 入力処理を最優先（Ctrl+C/Escの即座検出）
                Some(input) = self.input_stream.next() => {
                    if self.handle_input(input).await? {
                        break; // Quit signal
                    }
                }
                
                // 検索イベント処理
                Some(search_event) = self.search_stream.next() => {
                    self.handle_search_event(search_event).await?;
                }
                
                // UI更新（最低優先度）
                _ = self.tick_interval.tick() => {
                    self.render().await?;
                }
            }
        }
        
        // クリーンアップ
        self.cleanup().await?;
        
        Ok(())
    }
    
    /// Terminal クリーンアップ
    async fn cleanup(&mut self) -> Result<()> {
        use crossterm::{
            execute,
            terminal::{disable_raw_mode, LeaveAlternateScreen},
        };
        
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen
        )?;
        
        Ok(())
    }
    
    /// ユーザー入力を処理
    async fn handle_input(&mut self, input: InputEvent) -> Result<bool> {
        match input {
            InputEvent::Key(key) => {
                use crossterm::event::{KeyCode, KeyModifiers};
                
                match key {
                    // Ctrl+C または ESC で終了
                    KeyEvent { code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, .. }
                    | KeyEvent { code: KeyCode::Esc, .. } => {
                        return Ok(true); // Quit
                    }
                    
                    // 選択移動（Ctrl+N/P）
                    KeyEvent { code: KeyCode::Char('n'), modifiers: KeyModifiers::CONTROL, .. } => {
                        self.state.select_next();
                    }
                    KeyEvent { code: KeyCode::Char('p'), modifiers: KeyModifiers::CONTROL, .. } => {
                        self.state.select_previous();
                    }
                    
                    // 文字入力
                    KeyEvent { code: KeyCode::Char(c), .. } => {
                        let mut new_query = self.state.query.clone();
                        new_query.push(c);
                        self.state.update_query(new_query);
                        self.trigger_search().await?;
                    }
                    
                    // Backspace
                    KeyEvent { code: KeyCode::Backspace, .. } => {
                        let mut new_query = self.state.query.clone();
                        new_query.pop();
                        self.state.update_query(new_query);
                        if self.state.query.is_empty() {
                            self.state.results.clear();
                            self.state.loading = false;
                        } else {
                            self.trigger_search().await?;
                        }
                    }
                    
                    // 選択移動（矢印キー）
                    KeyEvent { code: KeyCode::Down, .. } => {
                        self.state.select_next();
                    }
                    KeyEvent { code: KeyCode::Up, .. } => {
                        self.state.select_previous();
                    }
                    
                    // Enter: ファイルを開く
                    KeyEvent { code: KeyCode::Enter, .. } => {
                        if let Some(result) = self.state.selected_result() {
                            self.open_file(&result.file_path).await?;
                        }
                    }
                    
                    _ => {}
                }
            }
            InputEvent::Resize(width, height) => {
                // ターミナルサイズ変更処理
                log::debug!("Terminal resized: {}x{}", width, height);
            }
            InputEvent::Mouse(_) => {
                // マウス処理（今のところ未実装）
            }
        }
        
        Ok(false) // Continue
    }
    
    /// 検索イベントを処理
    async fn handle_search_event(&mut self, event: SearchEvent) -> Result<()> {
        match event {
            SearchEvent::Started { query: _, mode: _ } => {
                self.state.loading = true;
                self.state.error_message = None;
            }
            SearchEvent::Results(results) => {
                self.state.results = results;
                self.state.selected_index = 0;
            }
            SearchEvent::Completed => {
                self.state.loading = false;
            }
            SearchEvent::Error(error) => {
                self.state.loading = false;
                self.state.error_message = Some(error);
            }
        }
        
        Ok(())
    }
    
    /// 検索をトリガー
    async fn trigger_search(&mut self) -> Result<()> {
        if self.state.query.trim().is_empty() {
            return Ok(());
        }
        
        let (response_tx, _response_rx) = oneshot::channel();
        
        // クリーンなクエリ（プレフィックスなし）を使用
        let clean_query = self.state.clean_query();
        
        let search_query = SearchQuery {
            query: clean_query,
            mode: self.state.search_mode.clone(),
            project_root: self.state.project_root.clone(),
            response_tx,
        };
        
        self.search_tx.send(search_query)?;
        Ok(())
    }
    
    /// ファイルを開く
    async fn open_file(&self, file_path: &PathBuf) -> Result<()> {
        // プラットフォーム別のファイルオープン
        #[cfg(target_os = "macos")]
        {
            tokio::process::Command::new("open")
                .arg(file_path)
                .spawn()?;
        }
        
        Ok(())
    }
    
    /// UI描画
    async fn render(&mut self) -> Result<()> {
        use ratatui::{
            widgets::{Block, Borders, List, ListItem, Paragraph},
            layout::{Layout, Constraint, Direction},
            style::{Color, Style},
        };
        
        self.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3), // 検索入力フィールド
                    Constraint::Min(0),    // 検索結果リスト
                    Constraint::Length(3), // ステータスバー
                ].as_ref())
                .split(f.size());
            
            // 検索入力フィールド
            let input_style = if self.state.loading {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            
            // クエリはそのまま表示（プレフィックスが含まれている）
            let input_text = self.state.query.clone();
            
            // モード情報をタイトルに表示
            let mode_name = match self.state.search_mode {
                SearchMode::Content => "Content",
                SearchMode::Symbol => "Symbol",
                SearchMode::File => "File",
                SearchMode::Regex => "Regex",
            };
            let title = format!(" Search ({}) ", mode_name);
            
            let input_block = Paragraph::new(input_text)
                .style(input_style)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .title(title));
            f.render_widget(input_block, chunks[0]);
            
            // 検索結果リスト
            let items: Vec<ListItem> = self.state.results
                .iter()
                .enumerate()
                .map(|(i, result)| {
                    let style = if i == self.state.selected_index {
                        Style::default().bg(Color::LightBlue).fg(Color::Black)
                    } else {
                        Style::default()
                    };
                    
                    let content = match &result.display_info {
                        crate::types::DisplayInfo::Content { line_content, .. } => {
                            format!("{}:{} {}", 
                                result.file_path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy(),
                                result.line,
                                line_content.trim())
                        }
                        crate::types::DisplayInfo::Symbol { name, symbol_type } => {
                            format!("{}:{} {:?} {}", 
                                result.file_path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy(),
                                result.line,
                                symbol_type,
                                name)
                        }
                        crate::types::DisplayInfo::File { path, .. } => {
                            format!("{}", path.display())
                        }
                        crate::types::DisplayInfo::Regex { line_content, .. } => {
                            format!("{}:{} {}", 
                                result.file_path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy(),
                                result.line,
                                line_content.trim())
                        }
                    };
                    
                    ListItem::new(content).style(style)
                })
                .collect();
            
            let results_list = List::new(items)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Results ({}) ", self.state.results.len())));
            f.render_widget(results_list, chunks[1]);
            
            // ステータスバー
            let status_text = if self.state.loading {
                "Searching...".to_string()
            } else if let Some(ref error) = self.state.error_message {
                format!("Error: {}", error)
            } else {
                format!("Mode: {:?} | {} results | ↑↓/C-p/C-n navigate | Enter open | Esc quit", 
                    self.state.search_mode, 
                    self.state.results.len())
            };
            
            let status_style = if self.state.error_message.is_some() {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Gray)
            };
            
            let status_block = Paragraph::new(status_text)
                .style(status_style)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .title(" Status "));
            f.render_widget(status_block, chunks[2]);
        })?;
        
        Ok(())
    }
}

/// TuiEngineのDropトレイト実装
impl Drop for TuiEngine {
    fn drop(&mut self) {
        // 同期的にクリーンアップを実行
        use crossterm::{
            execute,
            terminal::{disable_raw_mode, LeaveAlternateScreen},
        };
        
        // 可能な限りクリーンアップを試行
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen
        );
    }
}