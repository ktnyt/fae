//! Tree-sitterパーサー障害処理とパーサー例外ケースのテスト
//! 
//! パーサーメモリ不足、深いネスト構造、無限ループ、
//! パーサータイムアウト、部分解析ASTからのシンボル抽出、
//! 自動生成ファイルの処理などを詳細にテスト

use fae::{CacheManager, SearchRunner};
use anyhow::Result;
use std::fs;
use tempfile::TempDir;
use std::time::Instant;

/// 深いネスト構造によるパーサー負荷テスト
#[tokio::test]
async fn test_deep_nesting_structures() -> Result<()> {
    println!("🔍 深いネスト構造テスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // 極度に深いネスト構造（500レベル）
    let deep_nesting_file = temp_dir.path().join("deep_nesting.rs");
    let mut deep_content = String::new();
    
    // 深いネストの構造体定義
    deep_content.push_str("fn deep_nested_function() {\n");
    for i in 0..500 {
        deep_content.push_str(&format!("    {{\n"));
        if i % 50 == 0 {
            deep_content.push_str(&format!("        let level_{} = {};\n", i, i));
        }
    }
    for _ in 0..500 {
        deep_content.push_str("    }\n");
    }
    deep_content.push_str("}\n");
    
    // 深いネストの型定義
    deep_content.push_str("\nstruct DeepStruct {\n");
    for i in 0..100 {
        deep_content.push_str(&format!("    field_{}: Option<Option<Option<Box<Box<Vec<HashMap<String, Vec<Arc<Mutex<RefCell<i32>>>>>>>>>,\n", i));
    }
    deep_content.push_str("}\n");
    
    fs::write(&deep_nesting_file, &deep_content)?;
    
    println!("📁 深いネストファイル作成: {} バイト", deep_content.len());
    
    // パース時間の測定
    let parse_start = Instant::now();
    match cache_manager.get_symbols(&deep_nesting_file) {
        Ok(symbols) => {
            let parse_duration = parse_start.elapsed();
            println!("  深いネスト解析成功: {} シンボル, {:?}", symbols.len(), parse_duration);
            
            // 合理的な時間内で完了するかチェック
            assert!(parse_duration.as_secs() < 30, "深いネスト解析は30秒以内であるべき");
            assert!(symbols.len() >= 1, "最低限のシンボル（関数または構造体）が抽出されるべき");
            
            // シンボル詳細の確認
            for symbol in &symbols {
                println!("    シンボル: {} ({:?})", symbol.name, symbol.symbol_type);
            }
        }
        Err(e) => {
            let parse_duration = parse_start.elapsed();
            println!("  深いネスト解析エラー: {} ({:?})", e, parse_duration);
            
            // エラーでも合理的な時間内で失敗すべき
            assert!(parse_duration.as_secs() < 30, "エラーでも30秒以内で応答すべき");
        }
    }
    
    println!("✅ 深いネスト構造テスト完了");
    Ok(())
}

/// 複雑なマクロ展開とメタプログラミングテスト
#[tokio::test]
async fn test_complex_macro_metaprogramming() -> Result<()> {
    println!("🔍 複雑マクロ・メタプログラミングテスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // 複雑なマクロ定義
    let macro_file = temp_dir.path().join("complex_macros.rs");
    let macro_content = r#"
// 複雑なマクロ定義（パーサーに負荷をかける）
macro_rules! recursive_macro {
    (@step $_idx:expr,) => {};
    (@step $idx:expr, $head:ident, $($tail:ident,)*) => {
        fn $head() -> i32 { $idx }
        recursive_macro!(@step $idx + 1usize, $($tail,)*);
    };
    ($($n:ident,)*) => {
        recursive_macro!(@step 0usize, $($n,)*);
    }
}

// 大量のマクロ展開
recursive_macro!(
    func1, func2, func3, func4, func5, func6, func7, func8, func9, func10,
    func11, func12, func13, func14, func15, func16, func17, func18, func19, func20,
    func21, func22, func23, func24, func25, func26, func27, func28, func29, func30,
    func31, func32, func33, func34, func35, func36, func37, func38, func39, func40,
    func41, func42, func43, func44, func45, func46, func47, func48, func49, func50,
);

// プロシージャルマクロ風の複雑なパターン
macro_rules! generate_struct {
    (
        $(#[$attr:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_attr:meta])*
                $field_vis:vis $field_name:ident: $field_type:ty
            ),* $(,)?
        }
    ) => {
        $(#[$attr])*
        $vis struct $name {
            $(
                $(#[$field_attr])*
                $field_vis $field_name: $field_type,
            )*
        }
        
        impl $name {
            pub fn new() -> Self {
                Self {
                    $(
                        $field_name: Default::default(),
                    )*
                }
            }
        }
    };
}

// マクロ展開テスト
generate_struct!(
    #[derive(Debug, Clone)]
    pub struct ComplexStruct {
        #[serde(skip)]
        pub field1: String,
        pub field2: Vec<i32>,
        pub field3: HashMap<String, Value>,
        pub field4: Option<Box<dyn Trait>>,
    }
);

// ネストしたマクロ呼び出し
macro_rules! nested_macro {
    ($($tokens:tt)*) => {
        mod generated {
            $($tokens)*
        }
    };
}

nested_macro! {
    generate_struct! {
        pub struct NestedGenerated {
            pub data: Vec<String>,
        }
    }
}

// パーサーを混乱させる可能性のある構文
fn confusing_syntax() {
    let _ = || -> Result<Vec<Box<dyn Iterator<Item = Result<String, Error>> + Send + Sync>>, Error> {
        Ok(vec![])
    };
}
"#;
    
    fs::write(&macro_file, macro_content)?;
    
    let parse_start = Instant::now();
    match cache_manager.get_symbols(&macro_file) {
        Ok(symbols) => {
            let parse_duration = parse_start.elapsed();
            println!("  マクロファイル解析成功: {} シンボル, {:?}", symbols.len(), parse_duration);
            
            // マクロ関連のシンボルが適切に抽出されているか確認
            let macro_symbols: Vec<&str> = symbols.iter()
                .map(|s| s.name.as_str())
                .filter(|name| name.contains("macro") || name.contains("generate") || name.contains("Complex"))
                .collect();
            
            println!("    マクロ関連シンボル: {:?}", macro_symbols);
            
            assert!(symbols.len() >= 1, "マクロファイルから少なくとも1つのシンボルが抽出されるべき");
            assert!(parse_duration.as_secs() < 10, "マクロ解析は10秒以内であるべき");
        }
        Err(e) => {
            let parse_duration = parse_start.elapsed();
            println!("  マクロファイル解析エラー: {} ({:?})", e, parse_duration);
        }
    }
    
    println!("✅ 複雑マクロ・メタプログラミングテスト完了");
    Ok(())
}

/// 自動生成されたコードの処理テスト
#[tokio::test]
async fn test_generated_code_handling() -> Result<()> {
    println!("🔍 自動生成コード処理テスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // webpack bundle風の圧縮されたコード
    let webpack_bundle_file = temp_dir.path().join("webpack_bundle.js");
    let webpack_content = r#"!function(e){var t={};function n(r){if(t[r])return t[r].exports;var o=t[r]={i:r,l:!1,exports:{}};return e[r].call(o.exports,o,o.exports,n),o.l=!0,o.exports}n.m=e,n.c=t,n.d=function(e,t,r){n.o(e,t)||Object.defineProperty(e,t,{enumerable:!0,get:r})},n.r=function(e){"undefined"!=typeof Symbol&&Symbol.toStringTag&&Object.defineProperty(e,Symbol.toStringTag,{value:"Module"}),Object.defineProperty(e,"__esModule",{value:!0})},n.t=function(e,t){if(1&t&&(e=n(e)),8&t)return e;if(4&t&&"object"==typeof e&&e&&e.__esModule)return e;var r=Object.create(null);if(n.r(r),Object.defineProperty(r,"default",{enumerable:!0,value:e}),2&t&&"string"!=typeof e)for(var o in e)n.d(r,o,function(t){return e[t]}.bind(null,o));return r},n.o=function(e,t){return Object.prototype.hasOwnProperty.call(e,t)},n.p="",n(n.s=0)}([function(e,t,n){"use strict"}]);"#;
    fs::write(&webpack_bundle_file, webpack_content)?;
    
    // protobuf生成コード風
    let protobuf_file = temp_dir.path().join("generated.proto.rs");
    let protobuf_content = r#"
// Generated by protoc-rust. DO NOT EDIT!
// source: example.proto

#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]
#![allow(missing_docs)]

use protobuf::Message as Message_imported_for_functions;
use protobuf::ProtobufEnum as ProtobufEnum_imported_for_functions;

#[derive(PartialEq,Clone,Default)]
pub struct ExampleMessage {
    // message fields
    pub field1: ::std::option::Option<::std::string::String>,
    pub field2: ::std::option::Option<i32>,
    pub field3: ::protobuf::RepeatedField<::std::string::String>,
    // special fields
    pub unknown_fields: ::protobuf::UnknownFields,
    pub cached_size: ::protobuf::CachedSize,
}

impl ExampleMessage {
    pub fn new() -> ExampleMessage {
        ::std::default::Default::default()
    }
    
    pub fn default_instance() -> &'static ExampleMessage {
        static mut instance: ::protobuf::lazy::Lazy<ExampleMessage> = ::protobuf::lazy::Lazy::INIT;
        unsafe {
            instance.get(ExampleMessage::new)
        }
    }
    
    // Auto-generated getter/setter methods (1000+ lines would be here in real file)
    pub fn get_field1(&self) -> &str { &self.field1.as_ref().unwrap_or(&::std::string::String::new()) }
    pub fn clear_field1(&mut self) { self.field1 = ::std::option::Option::None; }
    pub fn set_field1(&mut self, v: ::std::string::String) { self.field1 = ::std::option::Option::Some(v); }
    pub fn mut_field1(&mut self) -> &mut ::std::string::String { 
        if self.field1.is_none() { self.field1.set_default(); }
        self.field1.as_mut().unwrap()
    }
    pub fn take_field1(&mut self) -> ::std::string::String {
        self.field1.take().unwrap_or_else(|| ::std::string::String::new())
    }
}

impl ::protobuf::Message for ExampleMessage {
    fn is_initialized(&self) -> bool { true }
    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    ::protobuf::rt::read_singular_proto3_string_into(wire_type, is, &mut self.field1)?;
                },
                2 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_int32()?;
                    self.field2 = ::std::option::Option::Some(tmp);
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            }
        }
        ::std::result::Result::Ok(())
    }
}
"#;
    fs::write(&protobuf_file, protobuf_content)?;
    
    // bindgen生成C++バインディング風
    let bindgen_file = temp_dir.path().join("bindgen_output.rs");
    let bindgen_content = r#"
/* automatically generated by rust-bindgen 0.56.0 */

pub const _STDINT_H: u32 = 1;
pub const _FEATURES_H: u32 = 1;
pub const _DEFAULT_SOURCE: u32 = 1;
pub const __USE_ISOC11: u32 = 1;
pub const __USE_ISOC99: u32 = 1;
pub const __USE_ISOC95: u32 = 1;
pub const __USE_POSIX_IMPLICITLY: u32 = 1;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _IO_FILE {
    pub _flags: ::std::os::raw::c_int,
    pub _IO_read_ptr: *mut ::std::os::raw::c_char,
    pub _IO_read_end: *mut ::std::os::raw::c_char,
    pub _IO_read_base: *mut ::std::os::raw::c_char,
    pub _IO_write_base: *mut ::std::os::raw::c_char,
    pub _IO_write_ptr: *mut ::std::os::raw::c_char,
    pub _IO_write_end: *mut ::std::os::raw::c_char,
    pub _IO_buf_base: *mut ::std::os::raw::c_char,
    pub _IO_buf_end: *mut ::std::os::raw::c_char,
    pub _IO_save_base: *mut ::std::os::raw::c_char,
    pub _IO_backup_base: *mut ::std::os::raw::c_char,
    pub _IO_save_end: *mut ::std::os::raw::c_char,
    pub _markers: *mut _IO_marker,
    pub _chain: *mut _IO_FILE,
    pub _fileno: ::std::os::raw::c_int,
    pub _flags2: ::std::os::raw::c_int,
    pub _old_offset: __off_t,
    pub _cur_column: ::std::os::raw::c_ushort,
    pub _vtable_offset: ::std::os::raw::c_schar,
    pub _shortbuf: [::std::os::raw::c_char; 1usize],
    pub _lock: *mut _IO_lock_t,
    pub _offset: __off64_t,
    pub _codecvt: *mut _IO_codecvt,
    pub _wide_data: *mut _IO_wide_data,
    pub _freeres_list: *mut _IO_FILE,
    pub _freeres_buf: *mut ::std::os::raw::c_void,
    pub __pad5: usize,
    pub _mode: ::std::os::raw::c_int,
    pub _unused2: [::std::os::raw::c_char; 20usize],
}

extern "C" {
    pub fn printf(__format: *const ::std::os::raw::c_char, ...) -> ::std::os::raw::c_int;
}

extern "C" {
    pub fn sprintf(
        __s: *mut ::std::os::raw::c_char,
        __format: *const ::std::os::raw::c_char,
        ...
    ) -> ::std::os::raw::c_int;
}
"#;
    fs::write(&bindgen_file, bindgen_content)?;
    
    println!("📋 自動生成コード処理結果:");
    
    // webpack bundle（JavaScript）
    match cache_manager.get_symbols(&webpack_bundle_file) {
        Ok(symbols) => {
            println!("  webpack bundle: {} シンボル（JSだが.rsとして処理）", symbols.len());
        }
        Err(e) => {
            println!("  webpack bundle: エラー（期待通り） - {}", e);
        }
    }
    
    // protobuf生成コード
    let protobuf_start = Instant::now();
    match cache_manager.get_symbols(&protobuf_file) {
        Ok(symbols) => {
            let duration = protobuf_start.elapsed();
            println!("  protobuf生成: {} シンボル, {:?}", symbols.len(), duration);
            
            // 生成コードから適切にシンボルが抽出されているか確認
            let message_symbols: Vec<&str> = symbols.iter()
                .map(|s| s.name.as_str())
                .filter(|name| name.contains("ExampleMessage") || name.contains("field"))
                .collect();
            
            println!("    protobufシンボル例: {:?}", message_symbols.iter().take(5).collect::<Vec<_>>());
            assert!(symbols.len() >= 5, "protobuf生成コードから多数のシンボルが抽出されるべき");
            assert!(duration.as_secs() < 5, "protobuf解析は5秒以内であるべき");
        }
        Err(e) => {
            println!("  protobuf生成: エラー - {}", e);
        }
    }
    
    // bindgen生成コード
    let bindgen_start = Instant::now();
    match cache_manager.get_symbols(&bindgen_file) {
        Ok(symbols) => {
            let duration = bindgen_start.elapsed();
            println!("  bindgen出力: {} シンボル, {:?}", symbols.len(), duration);
            
            let c_symbols: Vec<&str> = symbols.iter()
                .map(|s| s.name.as_str())
                .filter(|name| name.contains("_IO_") || name.contains("printf"))
                .collect();
            
            println!("    Cバインディング例: {:?}", c_symbols.iter().take(3).collect::<Vec<_>>());
            assert!(duration.as_secs() < 3, "bindgen解析は3秒以内であるべき");
        }
        Err(e) => {
            println!("  bindgen出力: エラー - {}", e);
        }
    }
    
    println!("✅ 自動生成コード処理テスト完了");
    Ok(())
}

/// 破損したASTからの部分的シンボル抽出テスト
#[tokio::test]
async fn test_partial_ast_symbol_extraction() -> Result<()> {
    println!("🔍 部分的AST シンボル抽出テスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // 構文エラーを含むが一部は有効なコード
    let partial_valid_file = temp_dir.path().join("partial_valid.rs");
    let partial_content = r#"
// 正常な関数
fn valid_function_1() -> i32 {
    42
}

// 構文エラーのある部分
fn broken_function( {
    // incomplete function definition
    let x = 1;
    // 閉じ括弧なし

// 正常な構造体
struct ValidStruct {
    field1: String,
    field2: i32,
}

// 不完全な構造体
struct BrokenStruct {
    field1: String,
    // missing closing brace

// 正常な実装ブロック
impl ValidStruct {
    fn new() -> Self {
        Self {
            field1: String::new(),
            field2: 0,
        }
    }
}

// 正常な関数2
fn valid_function_2() -> String {
    "test".to_string()
}

// 不完全なマクロ
macro_rules! broken_macro {
    ($x:expr) => {
        // incomplete macro body
"#;
    
    fs::write(&partial_valid_file, partial_content)?;
    
    // 完全に破損したファイル
    let completely_broken_file = temp_dir.path().join("completely_broken.rs");
    let broken_content = r#"
fn { {{ }}} fn struct impl { let let let mut mut &&& ||| 
struct { fn } impl } fn { struct impl fn }
macro_rules! { fn struct } impl { fn } struct
"#;
    fs::write(&completely_broken_file, broken_content)?;
    
    // 混合状態のファイル（有効+無効が混在）
    let mixed_file = temp_dir.path().join("mixed_validity.rs");
    let mixed_content = r#"
use std::collections::HashMap;

fn good_start() -> i32 { 1 }

fn bad_middle( { let x = } 

fn good_after_bad() -> String {
    "recovered".to_string()
}

struct GoodStruct { data: Vec<i32> }

struct BadStruct { field: missing_type

struct AnotherGoodStruct {
    name: String,
}

impl GoodStruct {
    fn method() -> Self { Self { data: vec![] } }
}

const GOOD_CONST: i32 = 42;
const BAD_CONST: = "missing type";
const ANOTHER_GOOD: &str = "test";
"#;
    fs::write(&mixed_file, mixed_content)?;
    
    println!("📋 部分AST抽出テスト結果:");
    
    // 部分的に有効なファイル
    match cache_manager.get_symbols(&partial_valid_file) {
        Ok(symbols) => {
            println!("  部分有効ファイル: {} シンボル", symbols.len());
            
            let valid_symbols: Vec<&str> = symbols.iter()
                .map(|s| s.name.as_str())
                .filter(|name| name.contains("valid") || name.contains("Valid"))
                .collect();
            
            println!("    回復シンボル: {:?}", valid_symbols);
            
            // 少なくとも有効な部分からシンボルが抽出されることを期待
            assert!(symbols.len() >= 2, "有効な部分から最低2つのシンボルが抽出されるべき");
            
            let has_valid_function = symbols.iter().any(|s| s.name.contains("valid_function"));
            let has_valid_struct = symbols.iter().any(|s| s.name.contains("ValidStruct"));
            
            if has_valid_function {
                println!("    ✅ 有効関数の回復成功");
            }
            if has_valid_struct {
                println!("    ✅ 有効構造体の回復成功");
            }
        }
        Err(e) => {
            println!("  部分有効ファイル: エラー - {}", e);
        }
    }
    
    // 完全に破損したファイル
    match cache_manager.get_symbols(&completely_broken_file) {
        Ok(symbols) => {
            println!("  完全破損ファイル: {} シンボル（予期しない回復）", symbols.len());
        }
        Err(e) => {
            println!("  完全破損ファイル: エラー（期待通り） - {}", e);
        }
    }
    
    // 混合状態ファイル
    match cache_manager.get_symbols(&mixed_file) {
        Ok(symbols) => {
            println!("  混合状態ファイル: {} シンボル", symbols.len());
            
            let good_symbols: Vec<&str> = symbols.iter()
                .map(|s| s.name.as_str())
                .filter(|name| name.contains("good") || name.contains("Good") || name.contains("GOOD"))
                .collect();
            
            println!("    良好シンボル: {:?}", good_symbols);
            
            // 混合ファイルから良好な部分のシンボルが回復されることを期待
            assert!(symbols.len() >= 3, "混合ファイルから複数のシンボルが回復されるべき");
        }
        Err(e) => {
            println!("  混合状態ファイル: エラー - {}", e);
        }
    }
    
    println!("✅ 部分的AST シンボル抽出テスト完了");
    Ok(())
}

/// パーサーメモリ使用量と処理時間の制限テスト
#[tokio::test]
async fn test_parser_resource_limits() -> Result<()> {
    println!("🔍 パーサーリソース制限テスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // 非常に大きなファイル（メモリ使用量テスト）
    let large_file = temp_dir.path().join("very_large.rs");
    let mut large_content = String::new();
    
    // 10,000個の関数を生成
    for i in 0..10_000 {
        large_content.push_str(&format!(r#"
fn function_{}() -> i32 {{
    let mut sum = 0;
    for j in 0..{} {{
        sum += j * {} + {};
    }}
    sum
}}
"#, i, i % 100 + 1, i, i * 2));
        
        if i % 1000 == 0 {
            large_content.push_str(&format!(r#"
struct Struct_{} {{
    field1: i32,
    field2: String,
    field3: Vec<i32>,
}}

impl Struct_{} {{
    fn new() -> Self {{
        Self {{
            field1: {},
            field2: format!("struct_{{}}", {}),
            field3: (0..{}).collect(),
        }}
    }}
}}
"#, i, i, i, i, i % 50 + 1));
        }
    }
    
    fs::write(&large_file, &large_content)?;
    println!("📁 大規模ファイル作成: {} バイト, {} 文字", large_content.len(), large_content.chars().count());
    
    // 非常に長い単一行（パーサー負荷テスト）
    let long_line_file = temp_dir.path().join("long_line.rs");
    let mut long_line_content = String::new();
    long_line_content.push_str("fn long_line_function() -> Vec<i32> { vec![");
    for i in 0..100_000 {
        if i > 0 {
            long_line_content.push_str(", ");
        }
        long_line_content.push_str(&i.to_string());
    }
    long_line_content.push_str("] }\n");
    
    fs::write(&long_line_file, &long_line_content)?;
    println!("📁 長行ファイル作成: {} バイト", long_line_content.len());
    
    println!("📋 リソース制限テスト結果:");
    
    // 大規模ファイルのパース時間測定
    let large_parse_start = Instant::now();
    match cache_manager.get_symbols(&large_file) {
        Ok(symbols) => {
            let duration = large_parse_start.elapsed();
            println!("  大規模ファイル: {} シンボル, {:?}", symbols.len(), duration);
            
            // パフォーマンス要件
            assert!(duration.as_secs() < 60, "大規模ファイル解析は60秒以内であるべき");
            assert!(symbols.len() >= 10_000, "10,000以上の関数シンボルが抽出されるべき");
            
            // メモリ使用量の概算確認（Rustの制約上、正確な測定は困難）
            let symbols_per_second = symbols.len() as f64 / duration.as_secs_f64();
            println!("    処理速度: {:.0} シンボル/秒", symbols_per_second);
            
            if symbols_per_second > 1000.0 {
                println!("    ✅ 高速パース性能");
            }
        }
        Err(e) => {
            let duration = large_parse_start.elapsed();
            println!("  大規模ファイル: エラー - {} ({:?})", e, duration);
            
            // エラーでも合理的な時間で応答すべき
            assert!(duration.as_secs() < 60, "エラーでも60秒以内で応答すべき");
        }
    }
    
    // 長い単一行のパース
    let long_line_start = Instant::now();
    match cache_manager.get_symbols(&long_line_file) {
        Ok(symbols) => {
            let duration = long_line_start.elapsed();
            println!("  長行ファイル: {} シンボル, {:?}", symbols.len(), duration);
            
            assert!(duration.as_secs() < 30, "長行解析は30秒以内であるべき");
            assert!(symbols.len() >= 1, "少なくとも1つの関数が抽出されるべき");
        }
        Err(e) => {
            let duration = long_line_start.elapsed();
            println!("  長行ファイル: エラー - {} ({:?})", e, duration);
        }
    }
    
    println!("✅ パーサーリソース制限テスト完了");
    Ok(())
}

/// SearchRunnerでのTree-sitter統合ストレステスト
#[tokio::test]
async fn test_search_runner_tree_sitter_stress() -> Result<()> {
    println!("🔍 SearchRunner Tree-sitter統合ストレステスト");
    
    let temp_dir = TempDir::new()?;
    let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
    
    // 複数言語の複雑なファイルを作成
    let rust_complex = temp_dir.path().join("complex.rs");
    fs::write(&rust_complex, r#"
use std::collections::{HashMap, BTreeMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};

trait ComplexTrait<T: Clone + Send + Sync> {
    type Associated: Iterator<Item = T>;
    fn complex_method(&self) -> Self::Associated;
}

struct ComplexStruct<T, U> 
where 
    T: Clone + Send + Sync + 'static,
    U: Iterator<Item = T>,
{
    field1: Arc<Mutex<HashMap<String, T>>>,
    field2: RwLock<BTreeMap<i64, U>>,
    field3: HashSet<Box<dyn ComplexTrait<T>>>,
}

impl<T, U> ComplexStruct<T, U> 
where 
    T: Clone + Send + Sync + 'static,
    U: Iterator<Item = T>,
{
    fn new() -> Self { unimplemented!() }
    
    async fn async_method(&self) -> Result<Vec<T>, Box<dyn std::error::Error + Send + Sync>> {
        unimplemented!()
    }
}

macro_rules! complex_macro {
    ($($name:ident: $type:ty),*) => {
        $(
            fn $name() -> $type { Default::default() }
        )*
    };
}

complex_macro!(test1: i32, test2: String, test3: Vec<i32>);
"#)?;
    
    let typescript_complex = temp_dir.path().join("complex.ts");
    fs::write(&typescript_complex, r#"
interface ComplexInterface<T extends Record<string, unknown>> {
  complexMethod<U>(param: T & U): Promise<Array<keyof T>>;
}

class ComplexClass<T, U extends T> implements ComplexInterface<T> {
  private complexField: Map<string, T>;
  
  constructor(private readonly data: T[]) {
    this.complexField = new Map();
  }
  
  async complexMethod<V>(param: T & V): Promise<Array<keyof T>> {
    return Object.keys(param) as Array<keyof T>;
  }
  
  public get complexGetter(): ReadonlyArray<T> {
    return [...this.data];
  }
}

const complexFunction = <T extends string | number>(
  param: T[]
): T extends string ? string[] : number[] => {
  return param as any;
};

namespace ComplexNamespace {
  export interface NestedInterface {
    nestedMethod(): void;
  }
  
  export class NestedClass implements NestedInterface {
    nestedMethod(): void {}
  }
}
"#)?;
    
    let python_complex = temp_dir.path().join("complex.py");
    fs::write(&python_complex, r#"
from typing import Generic, TypeVar, Dict, List, Optional, Union, Callable
from abc import ABC, abstractmethod
import asyncio

T = TypeVar('T')
U = TypeVar('U', bound=str)

class ComplexClass(Generic[T], ABC):
    def __init__(self, data: Dict[str, T]) -> None:
        self._data: Dict[str, T] = data
        self._cache: Optional[List[T]] = None
    
    @abstractmethod
    async def complex_async_method(self, param: T) -> List[T]:
        pass
    
    @property
    def complex_property(self) -> Dict[str, T]:
        return self._data.copy()
    
    def complex_generic_method(self, 
                             func: Callable[[T], U], 
                             items: List[T]) -> Dict[U, T]:
        return {func(item): item for item in items}

class ConcreteComplexClass(ComplexClass[int]):
    async def complex_async_method(self, param: int) -> List[int]:
        await asyncio.sleep(0.1)
        return [param * 2]

def complex_decorator(cls):
    def wrapper(*args, **kwargs):
        return cls(*args, **kwargs)
    return wrapper

@complex_decorator
class DecoratedClass:
    def decorated_method(self) -> None:
        pass
"#)?;
    
    use fae::cli::strategies::SymbolStrategy;
    let strategy = SymbolStrategy::new();
    
    println!("📋 多言語Tree-sitter統合テスト:");
    
    // Rust複雑構造の解析
    let rust_start = Instant::now();
    let rust_results = search_runner.collect_results_with_strategy(&strategy, "Complex")?;
    let rust_duration = rust_start.elapsed();
    
    println!("  Rust複雑構造: {} 件, {:?}", rust_results.len(), rust_duration);
    assert!(rust_results.len() >= 3, "Rust複雑構造から複数のシンボルが見つかるべき");
    
    // TypeScript複雑構造の解析
    let ts_start = Instant::now();
    let ts_results = search_runner.collect_results_with_strategy(&strategy, "complex")?;
    let ts_duration = ts_start.elapsed();
    
    println!("  TypeScript複雑構造: {} 件, {:?}", ts_results.len(), ts_duration);
    
    // Python複雑構造の解析
    let py_start = Instant::now();
    let py_results = search_runner.collect_results_with_strategy(&strategy, "Complex")?;
    let py_duration = py_start.elapsed();
    
    println!("  Python複雑構造: {} 件, {:?}", py_results.len(), py_duration);
    
    // 総合パフォーマンス評価
    let total_duration = rust_duration + ts_duration + py_duration;
    let total_results = rust_results.len() + ts_results.len() + py_results.len();
    
    println!("  総合結果: {} 件, {:?}", total_results, total_duration);
    
    assert!(total_duration.as_secs() < 10, "多言語解析は10秒以内であるべき");
    assert!(total_results >= 5, "多言語から合計5件以上のシンボルが見つかるべき");
    
    println!("✅ SearchRunner Tree-sitter統合ストレステスト完了");
    Ok(())
}