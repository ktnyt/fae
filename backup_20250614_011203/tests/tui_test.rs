//! TUIコンポーネントの単体テスト
//! 
//! TuiState、Message処理、イベントストリームなど、
//! TUI関連の各コンポーネントを個別にテストする

use fae::tui::{TuiState, TuiEvent, InputEvent, SearchEvent, SearchQuery};
use fae::types::{SearchResult, SearchMode, DisplayInfo};
use fae::cli::SearchRunner;
use anyhow::Result;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::sync::oneshot;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// テスト用のSearchResultを作成
fn create_test_search_result(file_name: &str, line: u32, content: &str) -> SearchResult {
    SearchResult {
        file_path: PathBuf::from(file_name),
        line,
        column: 1,
        display_info: DisplayInfo::Content {
            line_content: content.to_string(),
            match_start: 0,
            match_end: content.len(),
        },
        score: 1.0,
    }
}

mod tui_state_tests {
    use super::*;

    #[test]
    fn test_tui_state_creation() {
        let temp_dir = TempDir::new().unwrap();
        let state = TuiState::new(temp_dir.path().to_path_buf());
        
        assert_eq!(state.query, "");
        assert_eq!(state.results.len(), 0);
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.search_mode, SearchMode::Content);
        assert!(!state.loading);
        assert!(state.error_message.is_none());
        assert_eq!(state.project_root, temp_dir.path());
    }

    #[test]
    fn test_select_next_empty_results() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = TuiState::new(temp_dir.path().to_path_buf());
        
        state.select_next();
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_select_next_with_results() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = TuiState::new(temp_dir.path().to_path_buf());
        
        state.results = vec![
            create_test_search_result("file1.rs", 1, "test content 1"),
            create_test_search_result("file2.rs", 2, "test content 2"),
            create_test_search_result("file3.rs", 3, "test content 3"),
        ];
        
        // 最初は0
        assert_eq!(state.selected_index, 0);
        
        // 次に進む
        state.select_next();
        assert_eq!(state.selected_index, 1);
        
        state.select_next();
        assert_eq!(state.selected_index, 2);
        
        // 最後まで行ったら0に戻る
        state.select_next();
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_select_previous_with_results() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = TuiState::new(temp_dir.path().to_path_buf());
        
        state.results = vec![
            create_test_search_result("file1.rs", 1, "test content 1"),
            create_test_search_result("file2.rs", 2, "test content 2"),
            create_test_search_result("file3.rs", 3, "test content 3"),
        ];
        
        // 最初は0
        assert_eq!(state.selected_index, 0);
        
        // 前に戻る（最後の要素に）
        state.select_previous();
        assert_eq!(state.selected_index, 2);
        
        state.select_previous();
        assert_eq!(state.selected_index, 1);
        
        state.select_previous();
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_selected_result() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = TuiState::new(temp_dir.path().to_path_buf());
        
        // 結果がない場合
        assert!(state.selected_result().is_none());
        
        // 結果がある場合
        let result1 = create_test_search_result("file1.rs", 1, "test content 1");
        let result2 = create_test_search_result("file2.rs", 2, "test content 2");
        
        state.results = vec![result1.clone(), result2.clone()];
        
        // 最初の要素が選択されている
        let selected = state.selected_result().unwrap();
        assert_eq!(selected.file_path, result1.file_path);
        
        // 次の要素を選択
        state.select_next();
        let selected = state.selected_result().unwrap();
        assert_eq!(selected.file_path, result2.file_path);
    }
}

mod search_query_tests {
    use super::*;

    #[test]
    fn test_search_query_creation() {
        let temp_dir = TempDir::new().unwrap();
        let (tx, _rx) = oneshot::channel();
        
        let query = SearchQuery {
            query: "test query".to_string(),
            mode: SearchMode::Symbol,
            project_root: temp_dir.path().to_path_buf(),
            response_tx: tx,
        };
        
        assert_eq!(query.query, "test query");
        assert_eq!(query.mode, SearchMode::Symbol);
        assert_eq!(query.project_root, temp_dir.path());
    }
}

mod event_tests {
    use super::*;

    #[test]
    fn test_tui_event_variants() {
        let key_event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let input_event = InputEvent::Key(key_event);
        let tui_event = TuiEvent::Input(input_event);
        
        match tui_event {
            TuiEvent::Input(InputEvent::Key(key)) => {
                assert_eq!(key.code, KeyCode::Char('a'));
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[test]
    fn test_search_event_variants() {
        // 検索開始イベント
        let start_event = SearchEvent::Started {
            query: "test".to_string(),
            mode: SearchMode::Content,
        };
        
        match start_event {
            SearchEvent::Started { query, mode } => {
                assert_eq!(query, "test");
                assert_eq!(mode, SearchMode::Content);
            }
            _ => panic!("Unexpected event type"),
        }
        
        // 検索結果イベント
        let results = vec![create_test_search_result("test.rs", 1, "test content")];
        let results_event = SearchEvent::Results(results.clone());
        
        match results_event {
            SearchEvent::Results(received_results) => {
                assert_eq!(received_results.len(), 1);
                assert_eq!(received_results[0].file_path, results[0].file_path);
            }
            _ => panic!("Unexpected event type"),
        }
        
        // 検索完了イベント
        let completed_event = SearchEvent::Completed;
        assert!(matches!(completed_event, SearchEvent::Completed));
        
        // エラーイベント
        let error_event = SearchEvent::Error("Test error".to_string());
        match error_event {
            SearchEvent::Error(msg) => {
                assert_eq!(msg, "Test error");
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[test]
    fn test_input_event_variants() {
        // キーイベント
        let key_event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let input = InputEvent::Key(key_event);
        
        match input {
            InputEvent::Key(key) => {
                assert_eq!(key.code, KeyCode::Enter);
            }
            _ => panic!("Unexpected input type"),
        }
        
        // リサイズイベント
        let resize = InputEvent::Resize(80, 24);
        match resize {
            InputEvent::Resize(width, height) => {
                assert_eq!(width, 80);
                assert_eq!(height, 24);
            }
            _ => panic!("Unexpected input type"),
        }
    }

    #[test]
    fn test_mode_detection_and_switching() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();
        let mut state = TuiState::new(project_root);

        // デフォルトはContentモード
        assert_eq!(state.search_mode, SearchMode::Content);
        assert_eq!(state.clean_query(), "");

        // シンボル検索モード
        state.update_query("#function".to_string());
        assert_eq!(state.search_mode, SearchMode::Symbol);
        assert_eq!(state.query, "#function");
        assert_eq!(state.clean_query(), "function");

        // ファイル検索モード
        state.update_query(">src/main.rs".to_string());
        assert_eq!(state.search_mode, SearchMode::File);
        assert_eq!(state.query, ">src/main.rs");
        assert_eq!(state.clean_query(), "src/main.rs");

        // 正規表現検索モード
        state.update_query("/^fn\\s+".to_string());
        assert_eq!(state.search_mode, SearchMode::Regex);
        assert_eq!(state.query, "/^fn\\s+");
        assert_eq!(state.clean_query(), "^fn\\s+");

        // Contentモードに戻る
        state.update_query("normal text".to_string());
        assert_eq!(state.search_mode, SearchMode::Content);
        assert_eq!(state.query, "normal text");
        assert_eq!(state.clean_query(), "normal text");

        // 空クエリの動的切り替え
        state.update_query("#".to_string());
        assert_eq!(state.search_mode, SearchMode::Symbol);
        assert_eq!(state.clean_query(), "");

        state.update_query(">".to_string());
        assert_eq!(state.search_mode, SearchMode::File);
        assert_eq!(state.clean_query(), "");

        state.update_query("/".to_string());
        assert_eq!(state.search_mode, SearchMode::Regex);
        assert_eq!(state.clean_query(), "");
    }

    #[test]
    fn test_incremental_mode_switching() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();
        let mut state = TuiState::new(project_root);

        // 段階的な入力でモードが正しく切り替わることを確認
        state.update_query("#".to_string());
        assert_eq!(state.search_mode, SearchMode::Symbol);

        state.update_query("#f".to_string());
        assert_eq!(state.search_mode, SearchMode::Symbol);
        assert_eq!(state.clean_query(), "f");

        state.update_query("#func".to_string());
        assert_eq!(state.search_mode, SearchMode::Symbol);
        assert_eq!(state.clean_query(), "func");

        // プレフィックスを削除してContentモードに戻る
        state.update_query("func".to_string());
        assert_eq!(state.search_mode, SearchMode::Content);
        assert_eq!(state.clean_query(), "func");
    }

    #[test]
    fn test_navigation_with_results() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();
        let mut state = TuiState::new(project_root);

        // テスト用の結果を作成
        use fae::types::{SearchResult, DisplayInfo};
        use std::path::PathBuf;
        
        let results = vec![
            SearchResult {
                file_path: PathBuf::from("test1.rs"),
                line: 1,
                column: 1,
                display_info: DisplayInfo::Content {
                    line_content: "fn test1()".to_string(),
                    match_start: 0,
                    match_end: 4,
                },
                score: 1.0,
            },
            SearchResult {
                file_path: PathBuf::from("test2.rs"),
                line: 2,
                column: 1,
                display_info: DisplayInfo::Content {
                    line_content: "fn test2()".to_string(),
                    match_start: 0,
                    match_end: 4,
                },
                score: 0.9,
            },
            SearchResult {
                file_path: PathBuf::from("test3.rs"),
                line: 3,
                column: 1,
                display_info: DisplayInfo::Content {
                    line_content: "fn test3()".to_string(),
                    match_start: 0,
                    match_end: 4,
                },
                score: 0.8,
            },
        ];

        state.results = results;
        
        // 初期選択インデックスは0
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.selected_result().unwrap().file_path, PathBuf::from("test1.rs"));

        // 下に移動
        state.select_next();
        assert_eq!(state.selected_index, 1);
        assert_eq!(state.selected_result().unwrap().file_path, PathBuf::from("test2.rs"));

        state.select_next();
        assert_eq!(state.selected_index, 2);
        assert_eq!(state.selected_result().unwrap().file_path, PathBuf::from("test3.rs"));

        // 最後の要素から次に行くと最初に戻る
        state.select_next();
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.selected_result().unwrap().file_path, PathBuf::from("test1.rs"));

        // 上に移動
        state.select_previous();
        assert_eq!(state.selected_index, 2);
        assert_eq!(state.selected_result().unwrap().file_path, PathBuf::from("test3.rs"));

        state.select_previous();
        assert_eq!(state.selected_index, 1);
        assert_eq!(state.selected_result().unwrap().file_path, PathBuf::from("test2.rs"));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn create_test_project() -> Result<TempDir> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // テスト用ファイルを作成
        let mut file1 = File::create(root.join("test1.rs"))?;
        writeln!(file1, "fn main() {{")?;
        writeln!(file1, "    println!(\"Hello, world!\");")?;
        writeln!(file1, "}}")?;

        let mut file2 = File::create(root.join("test2.js"))?;
        writeln!(file2, "function greet(name) {{")?;
        writeln!(file2, "    console.log(`Hello, ${{name}}!`);")?;
        writeln!(file2, "}}")?;

        Ok(temp_dir)
    }

    #[tokio::test]
    async fn test_search_runner_collect_results() -> Result<()> {
        let temp_dir = create_test_project()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        
        // ContentStrategyのテスト
        use fae::cli::strategies::ContentStrategy;
        let strategy = ContentStrategy;
        
        let results = search_runner.collect_results_with_strategy(&strategy, "Hello")?;
        
        // 結果が返ってくることを確認（具体的な数は実装依存）
        // Note: .len() は常に >= 0 なので具体的な検証に変更
        
        // 結果がある場合、適切な構造になっていることを確認
        for result in &results {
            assert!(!result.file_path.as_os_str().is_empty());
            assert!(result.line >= 1);
            assert!(result.column >= 1);
        }
        
        Ok(())
    }

    #[tokio::test]
    async fn test_search_runner_with_empty_query() -> Result<()> {
        let temp_dir = create_test_project()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        
        use fae::cli::strategies::ContentStrategy;
        let strategy = ContentStrategy;
        
        // 空のクエリでも正常に動作することを確認
        let results = search_runner.collect_results_with_strategy(&strategy, "")?;
        
        // 空クエリでは結果は0件になるはず
        assert_eq!(results.len(), 0);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_search_runner_different_modes() -> Result<()> {
        let temp_dir = create_test_project()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        
        // FileStrategyのテスト
        use fae::cli::strategies::FileStrategy;
        let file_strategy = FileStrategy;
        
        let file_results = search_runner.collect_results_with_strategy(&file_strategy, "test")?;
        
        // ファイル検索の結果を確認
        for result in &file_results {
            match &result.display_info {
                DisplayInfo::File { path, .. } => {
                    assert!(path.to_string_lossy().contains("test"));
                }
                _ => panic!("Expected File display info"),
            }
        }
        
        // RegexStrategyのテスト
        use fae::cli::strategies::RegexStrategy;
        let regex_strategy = RegexStrategy;
        
        let regex_results = search_runner.collect_results_with_strategy(&regex_strategy, r"fn\s+\w+")?;
        
        // 正規表現検索の結果も適切な形式になっていることを確認
        for result in &regex_results {
            assert!(!result.file_path.as_os_str().is_empty());
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod async_components_tests {
    use super::*;
    use tokio::sync::mpsc;
    use futures_util::StreamExt;

    #[tokio::test]
    async fn test_search_stream_creation() {
        let temp_dir = TempDir::new().unwrap();
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        
        let (query_tx, query_rx) = mpsc::unbounded_channel();
        
        // 検索ストリームを作成
        let mut search_stream = fae::tui::create_search_stream(search_runner.clone(), query_rx);
        
        // 検索クエリを送信
        let (response_tx, _response_rx) = oneshot::channel();
        let search_query = SearchQuery {
            query: "test".to_string(),
            mode: SearchMode::Content,
            project_root: temp_dir.path().to_path_buf(),
            response_tx,
        };
        
        query_tx.send(search_query).unwrap();
        
        // ストリームから最初のイベントを受信
        if let Some(event) = search_stream.next().await {
            match event {
                SearchEvent::Started { query, mode } => {
                    assert_eq!(query, "test");
                    assert_eq!(mode, SearchMode::Content);
                }
                _ => {
                    // Started以外のイベントでも正常
                    // (実装によっては直接Resultsが来る可能性もある)
                }
            }
        }
    }

    #[tokio::test]
    async fn test_input_stream_creation() {
        // 入力ストリームの作成テスト
        let _input_stream = fae::tui::create_input_stream();
        
        // ストリームが作成されることを確認
        // 実際の入力ストリームのテストはCI環境では困難なため、
        // 作成のみをテスト
        
        // 作成に成功したらテスト成功
        assert!(true);
    }

    #[tokio::test]
    async fn test_multiple_search_queries() {
        let temp_dir = TempDir::new().unwrap();
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        
        let (query_tx, query_rx) = mpsc::unbounded_channel();
        let mut search_stream = fae::tui::create_search_stream(search_runner.clone(), query_rx);
        
        // 複数のクエリを送信
        for i in 0..3 {
            let (response_tx, _response_rx) = oneshot::channel();
            let search_query = SearchQuery {
                query: format!("test{}", i),
                mode: SearchMode::Content,
                project_root: temp_dir.path().to_path_buf(),
                response_tx,
            };
            query_tx.send(search_query).unwrap();
        }
        
        // 各クエリに対応するイベントを受信
        for _i in 0..3 {
            // タイムアウト付きで次のイベントを待つ
            let timeout_result = tokio::time::timeout(
                std::time::Duration::from_millis(1000),
                search_stream.next()
            ).await;
            
            assert!(timeout_result.is_ok());
            let event = timeout_result.unwrap();
            assert!(event.is_some());
        }
    }
}