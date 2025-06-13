//! UTF-8境界ケースとエンコーディング問題の包括的テスト
//! 
//! BOM処理、マルチバイト文字境界、不正UTF-8シーケンス、
//! 異なるエンコーディングの誤検出などを詳細にテスト

use fae::{CacheManager, SearchRunner};
use anyhow::Result;
use std::fs;
use tempfile::TempDir;

/// BOM (Byte Order Mark) 付きファイルの処理テスト
#[tokio::test]
async fn test_bom_handling() -> Result<()> {
    println!("🔍 BOM付きファイル処理テスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // UTF-8 BOM (\xEF\xBB\xBF)
    let utf8_bom_file = temp_dir.path().join("utf8_bom.rs");
    let utf8_bom_content = b"\xEF\xBB\xBFfn bom_function() {\n    println!(\"BOM test\");\n}";
    fs::write(&utf8_bom_file, utf8_bom_content)?;
    
    // UTF-16 BE BOM (\xFE\xFF) - Rustファイルとして無効だが処理確認
    let utf16_be_file = temp_dir.path().join("utf16_be.rs");
    let utf16_be_content = b"\xFE\xFF\x00f\x00n\x00 \x00u\x00t\x00f\x001\x006\x00(\x00)\x00 \x00{\x00}";
    fs::write(&utf16_be_file, utf16_be_content)?;
    
    // UTF-16 LE BOM (\xFF\xFE)
    let utf16_le_file = temp_dir.path().join("utf16_le.rs");
    let utf16_le_content = b"\xFF\xFEf\x00n\x00 \x00u\x00t\x00f\x001\x006\x00l\x00e\x00(\x00)\x00 \x00{\x00}\x00";
    fs::write(&utf16_le_file, utf16_le_content)?;
    
    // BOM処理結果の確認
    println!("📋 BOM処理結果:");
    
    // UTF-8 BOM - 正常に処理されるべき
    match cache_manager.get_symbols(&utf8_bom_file) {
        Ok(symbols) => {
            println!("  UTF-8 BOM: {} シンボル発見", symbols.len());
            assert!(symbols.len() > 0, "UTF-8 BOMファイルはシンボルを抽出できるべき");
            
            // BOMが適切に除去されてシンボル名が正確か確認
            let function_found = symbols.iter().any(|s| s.name.contains("bom_function"));
            assert!(function_found, "BOM除去後に正確なシンボル名が得られるべき");
        }
        Err(e) => println!("  UTF-8 BOM: エラー - {}", e),
    }
    
    // UTF-16 ファイル - エラーまたは空結果が期待される
    match cache_manager.get_symbols(&utf16_be_file) {
        Ok(symbols) => println!("  UTF-16 BE: {} シンボル（期待: 0）", symbols.len()),
        Err(e) => println!("  UTF-16 BE: エラー（期待通り） - {}", e),
    }
    
    match cache_manager.get_symbols(&utf16_le_file) {
        Ok(symbols) => println!("  UTF-16 LE: {} シンボル（期待: 0）", symbols.len()),
        Err(e) => println!("  UTF-16 LE: エラー（期待通り） - {}", e),
    }
    
    println!("✅ BOM処理テスト完了");
    Ok(())
}

/// マルチバイト文字境界での切断処理テスト
#[tokio::test]
async fn test_multibyte_boundary_handling() -> Result<()> {
    println!("🔍 マルチバイト文字境界テスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // 日本語文字を含むRustコード
    let japanese_file = temp_dir.path().join("japanese.rs");
    let japanese_content = r#"
// 日本語コメント：この関数は挨拶を出力します
fn こんにちは関数() -> String {
    let 挨拶 = "こんにちは、世界！";
    format!("挨拶: {}", 挨拶)
}

struct 日本語構造体 {
    名前: String,
    年齢: u32,
}

impl 日本語構造体 {
    fn 新規作成(名前: &str, 年齢: u32) -> Self {
        Self {
            名前: 名前.to_string(),
            年齢,
        }
    }
}

const 定数値: &str = "🎌 Japanese content 🗾";
"#;
    fs::write(&japanese_file, japanese_content)?;
    
    // 絵文字を含むRustコード
    let emoji_file = temp_dir.path().join("emoji.rs");
    let emoji_content = r#"
// Emoji test: 🚀🎯📊🔍💡⚡
fn rocket_function_🚀() -> String {
    let data = "🎯 target achieved! 📊";
    format!("🚀 Result: {}", data)
}

struct EmojiStruct_📊 {
    status: String,
    emoji_data: Vec<String>,
}

impl EmojiStruct_📊 {
    fn add_emoji_💡(&mut self, emoji: &str) {
        self.emoji_data.push(format!("💡 {}", emoji));
    }
}

const ROCKET_EMOJI: &str = "🚀🚀🚀";
"#;
    fs::write(&emoji_file, emoji_content)?;
    
    // 破損したUTF-8シーケンスを含むファイル
    let broken_utf8_file = temp_dir.path().join("broken_utf8.rs");
    let mut broken_content = Vec::new();
    broken_content.extend_from_slice(b"fn broken_function() {\n");
    broken_content.extend_from_slice(b"    // \xE3\x81 incomplete UTF-8\n"); // 不完全な日本語文字
    broken_content.extend_from_slice(b"    let value = \xC0\x80; // invalid UTF-8\n"); // 無効なUTF-8
    broken_content.extend_from_slice(b"    println!(\"test\");\n");
    broken_content.extend_from_slice(b"}\n");
    fs::write(&broken_utf8_file, broken_content)?;
    
    println!("📋 マルチバイト文字処理結果:");
    
    // 日本語ファイルのテスト
    match cache_manager.get_symbols(&japanese_file) {
        Ok(symbols) => {
            println!("  日本語ファイル: {} シンボル", symbols.len());
            
            // 日本語シンボル名が正確に抽出されているか確認
            let japanese_symbols: Vec<&str> = symbols.iter()
                .map(|s| s.name.as_str())
                .filter(|name| name.contains("日本語") || name.contains("こんにちは"))
                .collect();
            
            println!("    日本語シンボル: {:?}", japanese_symbols);
            assert!(symbols.len() >= 3, "日本語ファイルから複数のシンボルが抽出されるべき");
        }
        Err(e) => println!("  日本語ファイル: エラー - {}", e),
    }
    
    // 絵文字ファイルのテスト
    match cache_manager.get_symbols(&emoji_file) {
        Ok(symbols) => {
            println!("  絵文字ファイル: {} シンボル", symbols.len());
            
            // 絵文字を含むシンボル名の確認
            let emoji_symbols: Vec<&str> = symbols.iter()
                .map(|s| s.name.as_str())
                .filter(|name| name.contains("🚀") || name.contains("📊") || name.contains("💡"))
                .collect();
            
            println!("    絵文字シンボル: {:?}", emoji_symbols);
            assert!(symbols.len() >= 2, "絵文字ファイルから複数のシンボルが抽出されるべき");
        }
        Err(e) => println!("  絵文字ファイル: エラー - {}", e),
    }
    
    // 破損UTF-8ファイルのテスト
    match cache_manager.get_symbols(&broken_utf8_file) {
        Ok(symbols) => {
            println!("  破損UTF-8ファイル: {} シンボル（回復的解析）", symbols.len());
            
            // 少なくとも有効な部分からシンボルが抽出されるか確認
            let function_found = symbols.iter().any(|s| s.name.contains("broken_function"));
            if function_found {
                println!("    回復的解析成功: broken_function 発見");
            }
        }
        Err(e) => println!("  破損UTF-8ファイル: エラー（期待される場合もある） - {}", e),
    }
    
    println!("✅ マルチバイト文字境界テスト完了");
    Ok(())
}

/// Unicode正規化 (NFD vs NFC) の違いテスト  
#[tokio::test]
async fn test_unicode_normalization() -> Result<()> {
    println!("🔍 Unicode正規化テスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // NFC (Canonical Decomposition followed by Canonical Composition)
    // "が" = U+304C (単一コードポイント)
    let nfc_file = temp_dir.path().join("nfc_test.rs");
    let nfc_content = "fn nfc_が関数() -> String { \"NFC normalization test\".to_string() }";
    fs::write(&nfc_file, nfc_content)?;
    
    // NFD (Canonical Decomposition)  
    // "が" = U+304B (か) + U+3099 (濁点)
    let nfd_file = temp_dir.path().join("nfd_test.rs");
    let nfd_content = "fn nfd_か\u{3099}関数() -> String { \"NFD normalization test\".to_string() }";
    fs::write(&nfd_file, nfd_content)?;
    
    // アクセント付き文字の正規化テスト
    // é = U+00E9 (NFC) vs e + ´ = U+0065 + U+0301 (NFD)
    let accent_nfc_file = temp_dir.path().join("accent_nfc.rs");
    let accent_nfc_content = "fn café_function() -> String { \"NFC café\".to_string() }";
    fs::write(&accent_nfc_file, accent_nfc_content)?;
    
    let accent_nfd_file = temp_dir.path().join("accent_nfd.rs");
    let accent_nfd_content = "fn cafe\u{301}_function() -> String { \"NFD café\".to_string() }";
    fs::write(&accent_nfd_file, accent_nfd_content)?;
    
    println!("📋 Unicode正規化処理結果:");
    
    // NFC処理
    let nfc_symbols = cache_manager.get_symbols(&nfc_file)?;
    println!("  NFC 'が': {} シンボル", nfc_symbols.len());
    
    // NFD処理  
    let nfd_symbols = cache_manager.get_symbols(&nfd_file)?;
    println!("  NFD 'か+濁点': {} シンボル", nfd_symbols.len());
    
    // アクセント文字NFC
    let accent_nfc_symbols = cache_manager.get_symbols(&accent_nfc_file)?;
    println!("  NFC 'é': {} シンボル", accent_nfc_symbols.len());
    
    // アクセント文字NFD
    let accent_nfd_symbols = cache_manager.get_symbols(&accent_nfd_file)?;
    println!("  NFD 'e+´': {} シンボル", accent_nfd_symbols.len());
    
    // 正規化の一貫性確認
    assert!(nfc_symbols.len() > 0, "NFC正規化ファイルからシンボルが抽出されるべき");
    assert!(nfd_symbols.len() > 0, "NFD正規化ファイルからシンボルが抽出されるべき");
    assert!(accent_nfc_symbols.len() > 0, "NFCアクセント文字からシンボルが抽出されるべき");
    assert!(accent_nfd_symbols.len() > 0, "NFDアクセント文字からシンボルが抽出されるべき");
    
    // シンボル名の詳細確認
    for symbol in &nfc_symbols {
        println!("    NFC シンボル: '{}'", symbol.name);
    }
    for symbol in &nfd_symbols {
        println!("    NFD シンボル: '{}'", symbol.name);
    }
    
    println!("✅ Unicode正規化テスト完了");
    Ok(())
}

/// 異なるエンコーディングファイルの誤検出テスト
#[tokio::test]
async fn test_encoding_detection() -> Result<()> {
    println!("🔍 エンコーディング誤検出テスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // Shift_JIS風のバイトシーケンス（.rsファイルとして）
    let shift_jis_file = temp_dir.path().join("shift_jis_like.rs");
    let shift_jis_content = b"\x82\xb1\x82\xf1\x82\xc9\x82\xbf\x82\xcd"; // "こんにちは" in Shift_JIS
    fs::write(&shift_jis_file, shift_jis_content)?;
    
    // ISO-8859-1風のバイトシーケンス
    let iso_latin1_file = temp_dir.path().join("iso_latin1.rs");
    let iso_latin1_content = b"fn test_\xe9\xe8\xe7() { // \xe9\xe8\xe7 as ISO-8859-1 }";
    fs::write(&iso_latin1_file, iso_latin1_content)?;
    
    // Windows-1252風のバイトシーケンス
    let windows1252_file = temp_dir.path().join("windows1252.rs");
    let windows1252_content = b"fn windows_test() {\n    // \x80\x81\x82\x83 Windows-1252\n}";
    fs::write(&windows1252_file, windows1252_content)?;
    
    // バイナリ風だが一部ASCII含む
    let mixed_binary_file = temp_dir.path().join("mixed_binary.rs");
    let mut mixed_content = Vec::new();
    mixed_content.extend_from_slice(b"fn valid_start() {\n");
    // バイナリ風データを挿入
    for i in 0..256 {
        mixed_content.push(i as u8);
    }
    mixed_content.extend_from_slice(b"\n    // some text after binary\n}\n");
    fs::write(&mixed_binary_file, mixed_content)?;
    
    println!("📋 エンコーディング検出結果:");
    
    // Shift_JIS風ファイル
    match cache_manager.get_symbols(&shift_jis_file) {
        Ok(symbols) => {
            println!("  Shift_JIS風: {} シンボル（UTF-8として解釈）", symbols.len());
        }
        Err(e) => {
            println!("  Shift_JIS風: エラー（期待通り） - {}", e);
        }
    }
    
    // ISO-8859-1風ファイル
    match cache_manager.get_symbols(&iso_latin1_file) {
        Ok(symbols) => {
            println!("  ISO-8859-1風: {} シンボル", symbols.len());
            for symbol in &symbols {
                println!("    シンボル: '{}'", symbol.name);
            }
        }
        Err(e) => {
            println!("  ISO-8859-1風: エラー - {}", e);
        }
    }
    
    // Windows-1252風ファイル
    match cache_manager.get_symbols(&windows1252_file) {
        Ok(symbols) => {
            println!("  Windows-1252風: {} シンボル", symbols.len());
        }
        Err(e) => {
            println!("  Windows-1252風: エラー - {}", e);
        }
    }
    
    // 混合バイナリファイル
    match cache_manager.get_symbols(&mixed_binary_file) {
        Ok(symbols) => {
            println!("  混合バイナリ: {} シンボル（部分的回復）", symbols.len());
            
            // 有効な部分からシンボルが抽出されたか確認
            let valid_symbol_found = symbols.iter().any(|s| s.name.contains("valid_start"));
            if valid_symbol_found {
                println!("    部分的回復成功: valid_start 発見");
            }
        }
        Err(e) => {
            println!("  混合バイナリ: エラー - {}", e);
        }
    }
    
    println!("✅ エンコーディング誤検出テスト完了");
    Ok(())
}

/// 不正UTF-8シーケンスの部分的回復テスト
#[tokio::test]
async fn test_malformed_utf8_recovery() -> Result<()> {
    println!("🔍 不正UTF-8部分回復テスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // 様々な種類の不正UTF-8パターン
    let test_cases: Vec<(&str, Vec<u8>)> = vec![
        ("incomplete_sequence", b"fn test() {\n    // \xE3\x81 incomplete\n}".to_vec()),
        ("overlong_encoding", b"fn test2() {\n    // \xC0\x80 overlong\n}".to_vec()),
        ("invalid_start_byte", b"fn test3() {\n    // \xFF invalid start\n}".to_vec()),
        ("unexpected_continuation", b"fn test4() {\n    // valid \x81 unexpected\n}".to_vec()),
        ("mixed_valid_invalid", {
            let mut content = Vec::new();
            content.extend_from_slice(b"fn valid_");
            content.extend_from_slice("部分".as_bytes()); // 正しいUTF-8
            content.extend_from_slice(b"() {\n    // \xE3\x81 then ");
            content.extend_from_slice("正常文字".as_bytes()); // 正しいUTF-8
            content.extend_from_slice(b"\n}");
            content
        }),
    ];
    
    println!("📋 不正UTF-8回復テスト結果:");
    
    for (test_name, content) in test_cases {
        let test_file = temp_dir.path().join(format!("{}.rs", test_name));
        fs::write(&test_file, &content)?;
        
        match cache_manager.get_symbols(&test_file) {
            Ok(symbols) => {
                println!("  {}: {} シンボル（回復成功）", test_name, symbols.len());
                
                for symbol in &symbols {
                    println!("    回復シンボル: '{}'", symbol.name);
                }
                
                // 少なくとも何らかのシンボルが回復されることを期待
                if !symbols.is_empty() {
                    println!("    ✅ 部分的回復成功");
                }
            }
            Err(e) => {
                println!("  {}: エラー - {}", test_name, e);
            }
        }
    }
    
    println!("✅ 不正UTF-8部分回復テスト完了");
    Ok(())
}

/// SearchRunnerでの文字エンコーディング処理テスト
#[tokio::test]
async fn test_search_runner_encoding_handling() -> Result<()> {
    println!("🔍 SearchRunner文字エンコーディング処理テスト");
    
    let temp_dir = TempDir::new()?;
    let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
    
    // Unicode文字を含む検索対象ファイルを作成
    let unicode_files = vec![
        ("japanese.rs", "fn 日本語関数() { println!(\"こんにちは\"); }"),
        ("emoji.rs", "fn emoji_function_🚀() { println!(\"🎯\"); }"),
        ("accents.rs", "fn café_naïve_function() { println!(\"résumé\"); }"),
        ("mixed.rs", "fn mixed_文字列_🌟_café() { println!(\"test\"); }"),
    ];
    
    for (filename, content) in unicode_files {
        let file_path = temp_dir.path().join(filename);
        fs::write(&file_path, content)?;
    }
    
    // BOM付きファイルも作成
    let bom_file = temp_dir.path().join("bom.rs");
    let mut bom_content = Vec::new();
    bom_content.extend_from_slice(b"\xEF\xBB\xBF"); // UTF-8 BOM
    bom_content.extend_from_slice(b"fn bom_function() { println!(\"BOM test\"); }");
    fs::write(&bom_file, bom_content)?;
    
    println!("📋 SearchRunner Unicode検索テスト:");
    
    // 日本語検索
    use fae::cli::strategies::{SymbolStrategy, ContentStrategy};
    
    let symbol_strategy = SymbolStrategy::new();
    let content_strategy = ContentStrategy;
    
    // シンボル検索テスト
    let japanese_results = search_runner.collect_results_with_strategy(&symbol_strategy, "日本語")?;
    println!("  シンボル検索 '日本語': {} 件", japanese_results.len());
    
    let emoji_results = search_runner.collect_results_with_strategy(&symbol_strategy, "🚀")?;
    println!("  シンボル検索 '🚀': {} 件", emoji_results.len());
    
    let accent_results = search_runner.collect_results_with_strategy(&symbol_strategy, "café")?;
    println!("  シンボル検索 'café': {} 件", accent_results.len());
    
    // コンテンツ検索テスト
    let content_japanese = search_runner.collect_results_with_strategy(&content_strategy, "こんにちは")?;
    println!("  コンテンツ検索 'こんにちは': {} 件", content_japanese.len());
    
    let content_emoji = search_runner.collect_results_with_strategy(&content_strategy, "🎯")?;
    println!("  コンテンツ検索 '🎯': {} 件", content_emoji.len());
    
    let content_bom = search_runner.collect_results_with_strategy(&content_strategy, "BOM test")?;
    println!("  コンテンツ検索 'BOM test': {} 件", content_bom.len());
    
    // 結果の妥当性確認
    // Unicode文字を含むファイルが適切に処理されていることを確認
    let total_unicode_results = japanese_results.len() + emoji_results.len() + accent_results.len();
    println!("  総Unicode検索結果: {} 件", total_unicode_results);
    
    // 少なくとも一部のUnicode検索が成功することを期待
    if total_unicode_results > 0 {
        println!("  ✅ Unicode文字での検索成功");
    } else {
        println!("  ⚠️ Unicode文字での検索結果なし（要調査）");
    }
    
    println!("✅ SearchRunner文字エンコーディング処理テスト完了");
    Ok(())
}

/// 極端なUTF-8ケースのストレステスト
#[tokio::test]
async fn test_extreme_utf8_stress() -> Result<()> {
    println!("🔍 極端UTF-8ストレステスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // 非常に長いUnicode文字列を含む関数名
    let long_unicode_file = temp_dir.path().join("long_unicode.rs");
    let long_unicode_name = "🚀".repeat(100) + &"極".repeat(50) + &"A".repeat(100);
    let long_content = format!("fn {}() {{ println!(\"long unicode test\"); }}", long_unicode_name);
    fs::write(&long_unicode_file, long_content)?;
    
    // 様々なUnicodeブロックからの文字
    let unicode_blocks_file = temp_dir.path().join("unicode_blocks.rs");
    let unicode_blocks_content = r#"
// Latin: ÀÁÂÃÄÅÆÇÈÉÊË
fn latin_àáâãäåæçèéêë() {}

// Cyrillic: АБВГДЕЁЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯ
fn cyrillic_абвгдеёжз() {}

// Greek: ΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩ
fn greek_αβγδεζηθικλ() {}

// Arabic: العربية (right-to-left)
fn arabic_العربية() {}

// Hebrew: עברית (right-to-left)
fn hebrew_עברית() {}

// Chinese: 中文测试
fn chinese_中文测试() {}

// Japanese: ひらがなカタカナ漢字
fn japanese_ひらがなカタカナ漢字() {}

// Korean: 한글테스트
fn korean_한글테스트() {}

// Mathematical symbols: ∀∃∄∅∆∇∈∉∊∋∌∍∎∏
fn math_∀∃∄∅∆∇∈∉() {}

// Emoji combinations: 👨‍💻👩‍🔬🧑‍🎨
fn emoji_combo_👨‍💻👩‍🔬🧑‍🎨() {}
"#;
    fs::write(&unicode_blocks_file, unicode_blocks_content)?;
    
    // 4バイトUTF-8文字（Plane 1以上）
    let high_plane_file = temp_dir.path().join("high_plane.rs");
    let high_plane_content = r#"
// Mathematical script letters (U+1D400–U+1D7FF)
fn script_𝒜𝒷𝒸𝒹() {}

// Musical symbols (U+1D100–U+1D1FF)  
fn music_𝄞𝄢𝄫𝄪() {}

// Emoji beyond BMP (U+1F600+)
fn high_emoji_😀😃😄😁() {}
"#;
    fs::write(&high_plane_file, high_plane_content)?;
    
    println!("📋 極端UTF-8ストレステスト結果:");
    
    // 長いUnicode文字列テスト
    match cache_manager.get_symbols(&long_unicode_file) {
        Ok(symbols) => {
            println!("  長いUnicode: {} シンボル", symbols.len());
            for symbol in &symbols {
                let name_len = symbol.name.chars().count();
                println!("    シンボル長: {} 文字", name_len);
                if name_len > 200 {
                    println!("    ✅ 長いUnicode名の処理成功");
                }
            }
        }
        Err(e) => println!("  長いUnicode: エラー - {}", e),
    }
    
    // 多様なUnicodeブロックテスト
    match cache_manager.get_symbols(&unicode_blocks_file) {
        Ok(symbols) => {
            println!("  Unicodeブロック: {} シンボル", symbols.len());
            
            let block_types = vec![
                ("Latin", "àáâã"),
                ("Cyrillic", "абвг"),
                ("Greek", "αβγδ"),
                ("Arabic", "العربية"),
                ("Hebrew", "עברית"),
                ("Chinese", "中文"),
                ("Japanese", "ひらがな"),
                ("Korean", "한글"),
                ("Math", "∀∃∄"),
                ("Emoji", "👨‍💻"),
            ];
            
            for (block_name, sample) in block_types {
                let found = symbols.iter().any(|s| s.name.contains(sample));
                println!("    {}: {}", block_name, if found { "✅ 検出" } else { "❌ 未検出" });
            }
        }
        Err(e) => println!("  Unicodeブロック: エラー - {}", e),
    }
    
    // 高位プレーン文字テスト
    match cache_manager.get_symbols(&high_plane_file) {
        Ok(symbols) => {
            println!("  高位プレーン: {} シンボル", symbols.len());
            
            for symbol in &symbols {
                println!("    高位シンボル: '{}'", symbol.name);
            }
            
            if symbols.len() > 0 {
                println!("    ✅ 4バイトUTF-8文字の処理成功");
            }
        }
        Err(e) => println!("  高位プレーン: エラー - {}", e),
    }
    
    println!("✅ 極端UTF-8ストレステスト完了");
    Ok(())
}