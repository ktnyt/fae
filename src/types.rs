use std::path::PathBuf;
use std::time::SystemTime;

/// 検索モード
#[derive(Debug, Clone, PartialEq)]
pub enum SearchMode {
    Content,     // デフォルト
    Symbol,      // #prefix
    File,        // >prefix  
    Regex,       // /prefix
}

/// 検索結果の表示用データ
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// ファイルパス（絶対パス - 表示時に相対パス変換）
    pub file_path: PathBuf,
    /// 行番号（1ベース）
    pub line: u32,
    /// 列番号（1ベース）
    pub column: u32,
    /// 表示用のコンテキスト情報
    pub display_info: DisplayInfo,
    /// 検索スコア（ソート用）
    pub score: f64,
}

/// 表示用情報（検索モード別）
#[derive(Debug, Clone)]
pub enum DisplayInfo {
    /// コンテンツ検索の結果
    Content {
        /// ヒット箇所を含む行の内容
        line_content: String,
        /// ヒット開始位置（行内での文字位置）
        match_start: usize,
        /// ヒット終了位置
        match_end: usize,
    },
    /// シンボル検索の結果
    Symbol {
        /// シンボル名
        name: String,
        /// シンボルの種類
        symbol_type: SymbolType,
    },
    /// ファイル検索の結果
    File {
        /// 相対パス
        path: PathBuf,
        /// ディレクトリかどうか
        is_directory: bool,
    },
    /// 正規表現検索の結果
    Regex {
        /// ヒット箇所を含む行の内容
        line_content: String,
        /// マッチしたテキスト
        matched_text: String,
        /// ヒット開始位置
        match_start: usize,
        /// ヒット終了位置
        match_end: usize,
    },
}

/// シンボルの種類
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolType {
    Function,
    Class,
    Variable,
    Constant,
    Interface,
    Type,
}

impl SymbolType {
    /// 表示用アイコンを取得
    pub fn icon(&self) -> &'static str {
        match self {
            SymbolType::Function => "🔧",
            SymbolType::Class => "🏗️",
            SymbolType::Variable => "📦",
            SymbolType::Constant => "🔒",
            SymbolType::Interface => "🔌",
            SymbolType::Type => "📝",
        }
    }
}

/// キャッシュされたファイル情報
#[derive(Debug, Clone)]
pub struct CachedFileInfo {
    /// ファイルパス
    pub path: PathBuf,
    /// ファイルハッシュ（変更検知用）
    pub hash: u64,
    /// 最終更新時刻
    pub modified_time: SystemTime,
    /// ファイル内容（シンボル検索用にキャッシュ）
    pub content: Option<String>,
    /// 抽出されたシンボル
    pub symbols: Vec<CachedSymbol>,
    /// 最後にアクセスされた時刻（LRU用）
    pub last_accessed: SystemTime,
}

/// キャッシュされたシンボル情報
#[derive(Debug, Clone)]
pub struct CachedSymbol {
    /// シンボル名
    pub name: String,
    /// シンボルの種類
    pub symbol_type: SymbolType,
    /// 行番号（1ベース）
    pub line: u32,
    /// 列番号（1ベース）
    pub column: u32,
}

/// キャッシュエントリ（メモリ効率重視）
#[derive(Debug)]
pub struct CacheEntry {
    /// ファイル情報
    pub file_info: CachedFileInfo,
    /// メモリ使用量（バイト）
    pub memory_size: usize,
}

impl CacheEntry {
    /// キャッシュエントリの推定メモリサイズを計算
    pub fn estimate_memory_size(file_info: &CachedFileInfo) -> usize {
        let path_size = file_info.path.as_os_str().len();
        let content_size = file_info.content.as_ref().map_or(0, |c| c.len());
        let symbols_size = file_info.symbols.len() * 64; // 大まかな見積もり
        
        path_size + content_size + symbols_size + 128 // 固定オーバーヘッド
    }
}

/// 表示用のフォーマット済み検索結果
#[derive(Debug, Clone)]
pub struct FormattedResult {
    /// 左側（パスまたはシンボル名）
    pub left_part: String,
    /// 右側（プレビューまたはパス）
    pub right_part: String,
    /// 色分け情報
    pub color_info: ColorInfo,
}

/// 色分け情報
#[derive(Debug, Clone)]
pub struct ColorInfo {
    /// パス部分の色
    pub path_color: Color,
    /// 行/列番号の色
    pub location_color: Color,
    /// プレビュー/シンボル名の色
    pub content_color: Color,
    /// ハイライト部分の色
    pub highlight_color: Color,
}

/// 色の定義
#[derive(Debug, Clone)]
pub enum Color {
    Reset,
    Gray,
    Blue,
    Green,
    Yellow,
    Red,
    Cyan,
    White,
}