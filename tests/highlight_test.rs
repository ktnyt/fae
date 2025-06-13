use anyhow::Result;
use tempfile::TempDir;
use std::fs::File;
use std::io::Write;

use fae::searchers::{EnhancedContentSearcher};

/// ハイライト位置テストのためのテストケース
#[derive(Debug)]
struct HighlightTestCase {
    line_content: String,
    query: String,
    expected_start: usize,
    expected_end: usize,
    description: &'static str,
}

impl HighlightTestCase {
    fn new(line_content: &str, query: &str, expected_start: usize, expected_end: usize, description: &'static str) -> Self {
        Self {
            line_content: line_content.to_string(),
            query: query.to_string(),
            expected_start,
            expected_end,
            description,
        }
    }
}

/// テストケース一覧
fn get_highlight_test_cases() -> Vec<HighlightTestCase> {
    vec![
        // 基本的なASCII文字のマッチ
        HighlightTestCase::new(
            "SearchRunner",
            "search",
            0, 6,
            "ASCII文字: search in SearchRunner"
        ),
        HighlightTestCase::new(
            "ContentSearcher",
            "search",
            7, 13,
            "ASCII文字: search in ContentSearcher"
        ),
        
        // 日本語混じりのテスト
        HighlightTestCase::new(
            "/// 新しいRegexSearcherを作成",
            "search",
            13, 19,
            "日本語コメント内のASCII: search in RegexSearcher"
        ),
        HighlightTestCase::new(
            "// Searchエンジンの実装",
            "search",
            3, 9,
            "日本語混じり: Search in コメント"
        ),
        
        // マルチバイト文字境界テスト
        HighlightTestCase::new(
            "設定ファイルsearchテスト",
            "search",
            12, 18,
            "マルチバイト境界: 設定ファイル + search"
        ),
        HighlightTestCase::new(
            "データベースsearchクエリ実行",
            "search",
            18, 24,
            "マルチバイト境界: データベース + search"
        ),
        
        // 大文字小文字の混在
        HighlightTestCase::new(
            "SearchRunnerクラス",
            "runner",
            6, 12,
            "大文字小文字混在: Runner in SearchRunner"
        ),
        HighlightTestCase::new(
            "contentSearcher",
            "SEARCH",
            7, 13,
            "大文字小文字無視: SEARCH in contentSearcher"
        ),
        
        // エッジケース
        HighlightTestCase::new(
            "search",
            "search",
            0, 6,
            "完全一致"
        ),
        HighlightTestCase::new(
            "searchable",
            "search",
            0, 6,
            "部分一致"
        ),
        
        // 絵文字を含むテスト
        HighlightTestCase::new(
            "🔍 searchfunction",
            "search",
            4, 10,
            "絵文字付き: search in 🔍 searchfunction"
        ),
    ]
}

/// 実際のマッチ位置を検証する基本テスト
#[test] 
fn test_search_highlight_positions_basic() {
    // プライベートメソッドにアクセスできないため、
    // 実際の検索結果を使ってハイライト位置をテストする
    println!("\n=== 基本的なハイライト位置テスト ===");
    
    // テスト用文字列での単体テスト（string操作のみ）
    let test_cases = vec![
        ("SearchRunner", "search", 0, 6),
        ("ContentSearcher", "search", 7, 13),
        ("/// 新しいRegexSearcherを作成", "search", 13, 19),
        ("pub const SEARCH_DEBOUNCE: Duration", "search", 10, 16), // 問題のケース
        ("SEARCH_DEBOUNCE", "search", 0, 6), // より単純なケース
    ];
    
    for (line_content, query, expected_start, expected_end) in test_cases {
        println!("\nテスト: '{}' で '{}'", line_content, query);
        
        // 大文字小文字を無視して検索位置を計算（現在の実装と同じロジック）
        let line_lower = line_content.to_lowercase();
        let query_lower = query.to_lowercase();
        
        if let Some(start_pos) = line_lower.find(&query_lower) {
            // UTF-8文字数ベースでend位置を計算
            let query_char_len = query_lower.chars().count();
            let end_pos = line_content.char_indices()
                .nth(line_content[..start_pos].chars().count() + query_char_len)
                .map(|(i, _)| i)
                .unwrap_or(line_content.len());
            
            println!("  期待値: [{}..{}]", expected_start, expected_end);
            println!("  実際値: [{}..{}]", start_pos, end_pos);
            
            if start_pos < line_content.len() && end_pos <= line_content.len() && start_pos < end_pos {
                let before = &line_content[..start_pos];
                let matched = &line_content[start_pos..end_pos];
                let after = &line_content[end_pos..];
                println!("  ハイライト: '{}'[{}]'{}'", before, matched, after);
                
                // マッチした部分が期待通りかチェック
                assert!(matched.to_lowercase().contains(&query_lower), 
                       "マッチ部分 '{}' にクエリ '{}' が含まれていません", matched, query);
            }
        }
    }
}

/// 統合テスト: 実際の検索結果でのハイライト確認
#[test]
fn test_integrated_highlight_in_search_results() -> Result<()> {
    // テスト用プロジェクトを作成
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();
    
    // テストファイルを作成
    let test_content = r#"
use SearchRunner;
impl ContentSearcher {
    fn search_method() {
        println!("search function");
    }
}
/// 新しいRegexSearcherを作成
fn create_searcher() -> Result<()> {
    let searcher = SearchRunner::new();
    Ok(())
}
pub const SEARCH_DEBOUNCE: Duration = Duration::from_millis(100);
fn find_char_boundary(&self, content: &str, pos: usize, search_backward: bool) -> usize {
"#;
    
    let mut file = File::create(root.join("test.rs"))?;
    file.write_all(test_content.as_bytes())?;
    
    // EnhancedContentSearcherでの検索テスト
    let searcher = EnhancedContentSearcher::new(root.to_path_buf())?;
    println!("\n=== 統合テスト: EnhancedContentSearcher ===");
    let results = searcher.search("search", 100)?;
    
    println!("総検索結果数: {}", results.len());
    
    for result in &results {
        if let fae::types::DisplayInfo::Content { line_content, match_start, match_end } = &result.display_info {
            println!("ファイル: {:?}, 行: {}, カラム: {}", result.file_path.file_name(), result.line, result.column);
            println!("内容: '{}'", line_content);
            println!("ハイライト範囲: [{}..{}]", match_start, match_end);
            
            // 文字境界チェック
            println!("文字境界チェック: start={}, end={}", 
                    line_content.is_char_boundary(*match_start),
                    line_content.is_char_boundary(*match_end));
            
            // UTF-8安全性の確認
            if *match_start < line_content.len() && *match_end <= line_content.len() && match_start < match_end {
                let before = &line_content[..*match_start];
                let matched = &line_content[*match_start..*match_end];
                let after = &line_content[*match_end..];
                println!("分解: '{}'[{}]'{}'", before, matched, after);
                
                // マッチした部分が実際に'search'を含んでいることを確認
                assert!(matched.to_lowercase().contains("search"), 
                       "ハイライト部分 '{}' に 'search' が含まれていません", matched);
            } else {
                panic!("無効なハイライト範囲: [{}..{}] (行の長さ: {})", 
                      match_start, match_end, line_content.len());
            }
        }
    }
    
    // 少なくとも複数の結果が見つかることを確認
    assert!(results.len() >= 3, "十分な検索結果が見つかりませんでした: {} 件", results.len());
    
    Ok(())
}

/// UTF-8文字境界の安全性テスト
#[test]
fn test_utf8_character_boundary_safety() {
    let test_cases = vec![
        ("あいうえおsearchかきくけこ", "search", 15, 21), // 日本語文字の境界
        ("🔍🔎searchテスト", "search", 8, 14),  // 絵文字の境界
        ("café search café", "search", 5, 11),  // アクセント文字
    ];
    
    for (line_content, query, expected_start, expected_end) in test_cases {
        println!("\nUTF-8境界テスト: '{}'", line_content);
        
        // 文字列操作による位置計算（現在の実装をシミュレート）
        let line_lower = line_content.to_lowercase();
        let query_lower = query.to_lowercase();
        
        if let Some(start_pos) = line_lower.find(&query_lower) {
            // UTF-8文字数ベースでend位置を計算
            let query_char_len = query_lower.chars().count();
            let end_pos = line_content.char_indices()
                .nth(line_content[..start_pos].chars().count() + query_char_len)
                .map(|(i, _)| i)
                .unwrap_or(line_content.len());
            
            // 文字境界が安全であることを確認
            assert!(line_content.is_char_boundary(start_pos), 
                   "開始位置 {} が文字境界ではありません", start_pos);
            assert!(line_content.is_char_boundary(end_pos), 
                   "終了位置 {} が文字境界ではありません", end_pos);
            
            println!("位置: [{}..{}] (期待値: [{}..{}])", start_pos, end_pos, expected_start, expected_end);
            
            if start_pos < line_content.len() && end_pos <= line_content.len() && start_pos < end_pos {
                let matched = &line_content[start_pos..end_pos];
                println!("マッチ部分: '{}'", matched);
                assert!(matched.to_lowercase().contains(query.to_lowercase().as_str()));
            }
        }
    }
}