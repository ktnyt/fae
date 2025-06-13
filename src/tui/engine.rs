//! TUI: 非同期Iterator + メッセージエンジンアーキテクチャ
//! 
//! - ユーザー入力を非同期Streamで処理
//! - 検索バックエンドからの応答を非同期Streamで処理  
//! - メッセージエンジンが複数のStreamを統合してUI更新・検索リクエストを処理

use crate::types::{SearchResult, SearchMode};
use crate::cli::SearchRunner;
use crate::realtime_indexer::{RealtimeIndexer, FileChangeEvent, IndexUpdateResult};
use crate::cache_manager::CacheManager;

// 新しいTUIモジュールをインポート
use super::{TuiStyles, EditableText, InputHandler, InputResult, NavigationAction};
use super::constants::timing;

use anyhow::Result;
use crossterm::event::{Event as CrosstermEvent, KeyEvent, MouseEvent};
use futures_util::{Stream, StreamExt};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
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
    
    /// ファイル変更イベント（リアルタイム更新）
    FileChange(FileChangeEvent),
    
    /// インデックス更新完了通知
    IndexUpdate(IndexUpdateResult),
    
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
    pub cursor_position: usize, // 文字カーソル位置（UTF-8 文字境界）
    pub results: Vec<SearchResult>,
    pub selected_index: usize,
    pub search_mode: SearchMode,
    pub loading: bool,
    pub error_message: Option<String>,
    pub project_root: PathBuf,
    pub show_help: bool, // ヘルプオーバーレイ表示フラグ
}

impl TuiState {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            query: String::new(),
            cursor_position: 0,
            results: Vec::new(),
            selected_index: 0,
            search_mode: SearchMode::Content,
            loading: false,
            error_message: None,
            project_root,
            show_help: false,
        }
    }
    
    /// クエリを更新してモードを自動検出
    pub fn update_query(&mut self, new_query: String) {
        self.cursor_position = self.cursor_position.min(new_query.chars().count());
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

/// TuiStateにEditableTextトレイトを実装
impl EditableText for TuiState {
    fn text(&self) -> &str {
        &self.query
    }
    
    fn cursor_position(&self) -> usize {
        self.cursor_position
    }
    
    fn set_text_and_cursor(&mut self, text: String, cursor: usize) {
        self.query = text;
        self.cursor_position = cursor;
        self.detect_and_update_mode();
    }
}

impl TuiState {
    /// ヘルプオーバーレイの表示をトグル
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }
    
    /// 次の検索モードに切り替え（Tab）
    pub fn cycle_search_mode_forward(&mut self) {
        use crate::types::SearchMode;
        self.search_mode = match self.search_mode {
            SearchMode::Content => SearchMode::Symbol,
            SearchMode::Symbol => SearchMode::File,
            SearchMode::File => SearchMode::Regex,
            SearchMode::Regex => SearchMode::Content,
        };
        self.update_query_with_mode_prefix();
    }
    
    /// 前の検索モードに切り替え（Shift+Tab）
    pub fn cycle_search_mode_backward(&mut self) {
        use crate::types::SearchMode;
        self.search_mode = match self.search_mode {
            SearchMode::Content => SearchMode::Regex,
            SearchMode::Symbol => SearchMode::Content,
            SearchMode::File => SearchMode::Symbol,
            SearchMode::Regex => SearchMode::File,
        };
        self.update_query_with_mode_prefix();
    }
    
    /// 現在のクリーンクエリにモードプレフィックスを付けてqueryを更新
    fn update_query_with_mode_prefix(&mut self) {
        let clean_query = self.clean_query();
        let prefix = match self.search_mode {
            crate::types::SearchMode::Content => "",
            crate::types::SearchMode::Symbol => "#",
            crate::types::SearchMode::File => ">",
            crate::types::SearchMode::Regex => "/",
        };
        self.query = format!("{}{}", prefix, clean_query);
        self.cursor_position = self.query.chars().count();
    }
}

/// ユーザー入力を非同期Streamに変換
pub fn create_input_stream() -> impl Stream<Item = InputEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    
    // バックグラウンドタスクでcrossterm eventsを監視
    tokio::spawn(async move {
        loop {
            // 非ブロッキングでイベントをポーリング
            if crossterm::event::poll(timing::INPUT_POLL).unwrap_or(false) {
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
            tokio::time::sleep(timing::IDLE_POLL).await;
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
    // UI関連
    styles: TuiStyles,
    // リアルタイム機能
    cache_manager: Arc<Mutex<CacheManager>>,
    realtime_indexer: Option<RealtimeIndexer>,
}

impl TuiEngine {
    /// 新しいTuiEngineを作成
    pub fn new(project_root: PathBuf, search_runner: SearchRunner) -> Result<Self> {
        use crossterm::{
            execute,
            terminal::{enable_raw_mode, EnterAlternateScreen},
        };
        use ratatui::{backend::CrosstermBackend, Terminal};
        
        let state = TuiState::new(project_root.clone());
        
        // ユーザー入力ストリーム
        let input_stream = Box::pin(create_input_stream());
        
        // 検索ストリーム
        let (search_tx, search_rx) = mpsc::unbounded_channel();
        let search_stream = Box::pin(create_search_stream(search_runner, search_rx));
        
        // UIティックタイマー（60fps）  
        let tick_interval = tokio::time::interval(timing::UI_REFRESH);
        
        // Terminal初期化（一度だけ）
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        enable_raw_mode()?;
        
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        
        // リアルタイム機能の初期化
        let cache_manager = Arc::new(Mutex::new(CacheManager::new()));
        
        // RealtimeIndexerを作成（オプション、エラーが発生しても続行）
        let realtime_indexer = match RealtimeIndexer::new(project_root, cache_manager.clone()) {
            Ok(indexer) => {
                log::info!("RealtimeIndexer initialized successfully");
                Some(indexer)
            }
            Err(e) => {
                log::warn!("Failed to initialize RealtimeIndexer: {}", e);
                None
            }
        };
        
        Ok(Self {
            state,
            input_stream,
            search_stream,
            search_tx,
            tick_interval,
            terminal,
            styles: TuiStyles::default(),
            cache_manager,
            realtime_indexer,
        })
    }
    
    /// リアルタイムインデックスのイベントループを開始
    pub async fn start_realtime_indexing(&mut self) -> Result<()> {
        if let Some(realtime_indexer) = self.realtime_indexer.take() {
            // RealtimeIndexerのイベントループを別タスクで開始
            let _cache_manager = self.cache_manager.clone();
            tokio::spawn(async move {
                let mut indexer = realtime_indexer;
                if let Err(e) = indexer.start_event_loop().await {
                    log::error!("RealtimeIndexer event loop failed: {}", e);
                }
            });
            log::info!("Started realtime indexing in background");
        }
        Ok(())
    }

    /// メインイベントループ
    pub async fn run(&mut self) -> Result<()> {
        // リアルタイムインデックスを開始
        self.start_realtime_indexing().await?;
        
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
                // 借用問題を避けるため、値をコピー
                let current_mode = self.state.search_mode.clone();
                let help_visible = self.state.show_help;
                let result_count = self.state.results.len();
                
                // 新しいInputHandlerを使用
                let result = InputHandler::handle_key(
                    &mut self.state,
                    key,
                    current_mode,
                    help_visible,
                    result_count
                );
                
                match result {
                    InputResult::Quit => return Ok(true),
                    InputResult::ToggleHelp => {
                        self.state.toggle_help();
                    }
                    InputResult::SelectResult => {
                        if let Some(result) = self.state.selected_result() {
                            self.open_file(&result.file_path).await?;
                        }
                    }
                    InputResult::ModeChanged(new_mode) => {
                        self.state.search_mode = new_mode;
                        self.state.update_query_with_mode_prefix();
                        self.trigger_search_if_needed().await?;
                    }
                    InputResult::QueryUpdated => {
                        self.trigger_search_if_needed().await?;
                    }
                    InputResult::Navigate(action) => {
                        match action {
                            NavigationAction::Up => self.state.select_previous(),
                            NavigationAction::Down => self.state.select_next(),
                            NavigationAction::PageUp => {
                                // 10項目上に移動
                                for _ in 0..10 {
                                    self.state.select_previous();
                                }
                            }
                            NavigationAction::PageDown => {
                                // 10項目下に移動
                                for _ in 0..10 {
                                    self.state.select_next();
                                }
                            }
                            NavigationAction::Home => {
                                self.state.selected_index = 0;
                            }
                            NavigationAction::End => {
                                if !self.state.results.is_empty() {
                                    self.state.selected_index = self.state.results.len() - 1;
                                }
                            }
                        }
                    }
                    InputResult::Continue | InputResult::Unhandled => {
                        // 何もしない
                    }
                }
            }
            InputEvent::Resize(width, height) => {
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
    
    /// 検索が必要な場合のみトリガー（空の場合は結果をクリア）
    async fn trigger_search_if_needed(&mut self) -> Result<()> {
        if self.state.query.trim().is_empty() {
            self.state.results.clear();
            self.state.loading = false;
            Ok(())
        } else {
            self.trigger_search().await
        }
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
            style::{Color, Style, Modifier},
            text::{Line, Span},
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
            
            // 検索入力フィールド（スタイル統一）
            let (mode_name, mode_style) = match self.state.search_mode {
                SearchMode::Content => ("Content", self.styles.mode_content),
                SearchMode::Symbol => ("Symbol", self.styles.mode_symbol),
                SearchMode::File => ("File", self.styles.mode_file),
                SearchMode::Regex => ("Regex", self.styles.mode_regex),
            };
            
            let input_style = if self.state.loading {
                self.styles.loading
            } else {
                mode_style
            };
            
            // クエリの色分け表示
            let input_spans = if !self.state.query.is_empty() {
                match self.state.search_mode {
                    SearchMode::Content => {
                        vec![Span::styled(self.state.query.clone(), input_style)]
                    }
                    SearchMode::Symbol => {
                        vec![
                            Span::styled("#", self.styles.prefix_symbol),
                            Span::styled(self.state.clean_query(), input_style),
                        ]
                    }
                    SearchMode::File => {
                        vec![
                            Span::styled(">", self.styles.prefix_file),
                            Span::styled(self.state.clean_query(), input_style),
                        ]
                    }
                    SearchMode::Regex => {
                        vec![
                            Span::styled("/", self.styles.prefix_regex),
                            Span::styled(self.state.clean_query(), input_style),
                        ]
                    }
                }
            } else {
                vec![Span::styled("", input_style)]
            };
            
            let title = format!(" Search ({}) ", mode_name);
            let title_block = Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(title, mode_style));
            
            let input_block = Paragraph::new(Line::from(input_spans))
                .block(title_block);
            f.render_widget(input_block, chunks[0]);
            
            // 検索結果リスト（色分け対応）
            let items: Vec<ListItem> = self.state.results
                .iter()
                .enumerate()
                .map(|(i, result)| {
                    let is_selected = i == self.state.selected_index;
                    
                    // ファイルパスを相対パスに変換
                    let relative_path = result.file_path
                        .strip_prefix(&self.state.project_root)
                        .unwrap_or(&result.file_path)
                        .to_string_lossy();
                    
                    // 結果の種類に応じて色分けされたSpansを作成
                    let spans = match &result.display_info {
                        crate::types::DisplayInfo::Content { line_content, .. } => {
                            vec![
                                Span::styled(
                                    format!("{}", relative_path),
                                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                                ),
                                Span::styled(
                                    format!(":{}", result.line),
                                    Style::default().fg(Color::Yellow)
                                ),
                                Span::raw(" "),
                                Span::styled(
                                    line_content.trim().to_string(),
                                    Style::default().fg(Color::White)
                                ),
                            ]
                        }
                        crate::types::DisplayInfo::Symbol { name, symbol_type } => {
                            // シンボルタイプ別の色分け
                            let symbol_color = match symbol_type {
                                crate::types::SymbolType::Function => Color::Green,
                                crate::types::SymbolType::Class => Color::Blue,
                                crate::types::SymbolType::Variable => Color::Magenta,
                                crate::types::SymbolType::Constant => Color::Red,
                                crate::types::SymbolType::Interface => Color::Cyan,
                                crate::types::SymbolType::Type => Color::Yellow,
                            };
                            
                            vec![
                                Span::styled(
                                    format!("{}", relative_path),
                                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                                ),
                                Span::styled(
                                    format!(":{}", result.line),
                                    Style::default().fg(Color::Yellow)
                                ),
                                Span::raw(" "),
                                Span::styled(
                                    format!("{:?}", symbol_type),
                                    Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC)
                                ),
                                Span::raw(" "),
                                Span::styled(
                                    name.clone(),
                                    Style::default().fg(symbol_color).add_modifier(Modifier::BOLD)
                                ),
                            ]
                        }
                        crate::types::DisplayInfo::File { path, is_directory } => {
                            let color = if *is_directory { Color::Blue } else { Color::Cyan };
                            let modifier = if *is_directory { Modifier::BOLD } else { Modifier::empty() };
                            
                            vec![
                                Span::styled(
                                    format!("{}", path.display()),
                                    Style::default().fg(color).add_modifier(modifier)
                                ),
                            ]
                        }
                        crate::types::DisplayInfo::Regex { line_content, matched_text, .. } => {
                            // マッチ部分をハイライト
                            let content = line_content.trim();
                            let highlighted_content = if let Some(pos) = content.find(matched_text) {
                                let before = &content[..pos];
                                let matched = &content[pos..pos + matched_text.len()];
                                let after = &content[pos + matched_text.len()..];
                                
                                vec![
                                    Span::raw(before.to_string()),
                                    Span::styled(
                                        matched.to_string(),
                                        Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
                                    ),
                                    Span::raw(after.to_string()),
                                ]
                            } else {
                                vec![Span::raw(content.to_string())]
                            };
                            
                            let mut result_spans = vec![
                                Span::styled(
                                    format!("{}", relative_path),
                                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                                ),
                                Span::styled(
                                    format!(":{}", result.line),
                                    Style::default().fg(Color::Yellow)
                                ),
                                Span::raw(" "),
                            ];
                            result_spans.extend(highlighted_content);
                            result_spans
                        }
                    };
                    
                    // 選択状態の背景色を適用
                    let final_spans = if is_selected {
                        spans.into_iter()
                            .map(|span| {
                                Span::styled(
                                    span.content,
                                    span.style.bg(Color::LightBlue).fg(Color::Black)
                                )
                            })
                            .collect()
                    } else {
                        spans
                    };
                    
                    ListItem::new(Line::from(final_spans))
                })
                .collect();
            
            let results_count = self.state.results.len();
            let results_title = if results_count > 0 {
                Span::styled(
                    format!(" Results ({}) ", results_count),
                    self.styles.results_title
                )
            } else {
                Span::styled(
                    " Results (0) ",
                    self.styles.results_empty
                )
            };
            
            let results_list = List::new(items)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .title(results_title));
            f.render_widget(results_list, chunks[1]);
            
            // ステータスバー（色分け対応）
            let status_spans = if self.state.loading {
                vec![
                    Span::styled("Searching", Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK)),
                    Span::styled("...", Style::default().fg(Color::Yellow)),
                ]
            } else if let Some(ref error) = self.state.error_message {
                vec![
                    Span::styled("Error: ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                    Span::styled(error.clone(), Style::default().fg(Color::Red)),
                ]
            } else {
                vec![
                    Span::styled("Mode: ", Style::default().fg(Color::Gray)),
                    Span::styled(format!("{:?}", self.state.search_mode), mode_style),
                    Span::styled(" | ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{}", self.state.results.len()),
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    ),
                    Span::styled(" results | ", Style::default().fg(Color::Gray)),
                    
                    // Basic keys
                    Span::styled("↑↓", Style::default().fg(Color::Green)),
                    Span::styled(" nav | ", Style::default().fg(Color::Gray)),
                    Span::styled("Tab", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(" mode | ", Style::default().fg(Color::Gray)),
                    Span::styled("Enter", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                    Span::styled(" open | ", Style::default().fg(Color::Gray)),
                    Span::styled("?", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(" help | ", Style::default().fg(Color::Gray)),
                    Span::styled("Esc", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                    Span::styled(" quit", Style::default().fg(Color::Gray)),
                ]
            };
            
            let status_title_style = if self.state.error_message.is_some() {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if self.state.loading {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            
            let status_block = Paragraph::new(Line::from(status_spans))
                .block(Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(" Status ", status_title_style)));
            f.render_widget(status_block, chunks[2]);
            
            // ヘルプオーバーレイの表示
            if self.state.show_help {
                Self::render_help_overlay(f);
            }
        })?;
        
        Ok(())
    }
    
    /// ヘルプオーバーレイを描画
    fn render_help_overlay(f: &mut ratatui::Frame) {
        use ratatui::{
            widgets::{Block, Borders, Paragraph, Clear},
            layout::Alignment,
            style::{Color, Style, Modifier},
            text::{Line, Span},
        };
        
        // 画面サイズの70%のサイズでセンタリング
        let size = f.size();
        let popup_area = ratatui::layout::Rect {
            x: size.width / 6,
            y: size.height / 8,
            width: size.width * 2 / 3,
            height: size.height * 3 / 4,
        };
        
        // 背景をクリア
        f.render_widget(Clear, popup_area);
        
        // ヘルプコンテンツを作成
        let help_lines = vec![
            Line::from(vec![
                Span::styled("fae - Fast And Elegant Search", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Search Modes:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ]),
            Line::from(vec![
                Span::styled("  Content  ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::styled("Normal text search (default)", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  #symbol  ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled("Search functions, classes, variables", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  >file    ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                Span::styled("Search file names and paths", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  /regex   ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::styled("Regular expression search", Style::default().fg(Color::Gray))
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Mode Switching:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ]),
            Line::from(vec![
                Span::styled("  Tab      ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled("Cycle to next search mode", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Shift+Tab", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled("Cycle to previous search mode", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+]/[ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled("Alternative mode cycling (forward/backward)", Style::default().fg(Color::Gray))
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Navigation:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ]),
            Line::from(vec![
                Span::styled("  ↑↓       ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled("Navigate search results", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+P/N ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled("Navigate search results (Emacs style)", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Enter    ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                Span::styled("Open selected file", Style::default().fg(Color::Gray))
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Cursor Movement:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ]),
            Line::from(vec![
                Span::styled("  ←→       ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled("Move cursor left/right", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+A   ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled("Move to beginning of line", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+E   ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled("Move to end of line", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+F   ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled("Move forward one character", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+B   ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled("Move backward one character", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Home/End ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled("Move to beginning/end of line", Style::default().fg(Color::Gray))
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Editing:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ]),
            Line::from(vec![
                Span::styled("  Backspace", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled("Delete character before cursor", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Delete   ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled("Delete character at cursor", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+H   ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled("Delete character before cursor", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+D   ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled("Delete character at cursor", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+K   ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled("Delete from cursor to end of line", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+U   ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled("Clear entire line", Style::default().fg(Color::Gray))
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Other:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ]),
            Line::from(vec![
                Span::styled("  ?        ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled("Show this help", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Esc      ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::styled("Close help or quit application", Style::default().fg(Color::Gray))
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+C   ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::styled("Quit application", Style::default().fg(Color::Gray))
            ]),
        ];
        
        let help_paragraph = Paragraph::new(help_lines)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" Help (Press ? or Esc to close) ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
                .style(Style::default().bg(Color::Black)))
            .alignment(Alignment::Left)
            .wrap(ratatui::widgets::Wrap { trim: false });
        
        f.render_widget(help_paragraph, popup_area);
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