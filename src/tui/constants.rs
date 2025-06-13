//! TUI定数定義
//! 
//! タイミング、レイアウト、動作に関する定数値

use std::time::Duration;

/// タイミング関連の定数
pub mod timing {
    use super::Duration;
    
    /// ユーザー入力ポーリング間隔
    pub const INPUT_POLL: Duration = Duration::from_millis(50);
    
    /// 非入力時のポーリング間隔（省エネ）
    pub const IDLE_POLL: Duration = Duration::from_millis(10);
    
    /// UI再描画レート（60fps）
    pub const UI_REFRESH: Duration = Duration::from_millis(16);
    
    /// 検索結果の更新デバウンス
    pub const SEARCH_DEBOUNCE: Duration = Duration::from_millis(100);
    
    /// エラーメッセージ表示時間
    pub const ERROR_DISPLAY: Duration = Duration::from_secs(3);
}

/// UIレイアウト関連の定数
pub mod layout {
    /// ヘルプオーバーレイの幅比率
    pub const HELP_WIDTH_RATIO: f32 = 2.0 / 3.0;
    
    /// ヘルプオーバーレイの高さ比率
    pub const HELP_HEIGHT_RATIO: f32 = 3.0 / 4.0;
    
    /// 結果リストの最大表示数
    pub const MAX_VISIBLE_RESULTS: usize = 1000;
    
    /// スクロール時の移動量
    pub const SCROLL_AMOUNT: usize = 5;
    
    /// 検索入力フィールドの最小幅
    pub const MIN_INPUT_WIDTH: u16 = 20;
    
    /// ステータスバーの高さ
    pub const STATUS_BAR_HEIGHT: u16 = 1;
    
    /// 入力フィールドの高さ
    pub const INPUT_HEIGHT: u16 = 3;
}

/// キーボードショートカット関連
pub mod shortcuts {
    /// ヘルプ表示の切り替えキー
    pub const HELP_KEY: char = '?';
    
    /// 検索モード切り替えキー
    pub const MODE_CYCLE_KEY: char = '\t';
    
    /// 結果選択時のコピーキー
    pub const COPY_KEY: char = '\n'; // Enter
    
    /// アプリケーション終了キー
    pub const QUIT_KEY: char = 'q';
}

/// 検索と処理関連の定数
pub mod search {
    /// 検索クエリの最大長
    pub const MAX_QUERY_LENGTH: usize = 500;
    
    /// 検索結果の最大件数
    pub const MAX_RESULTS: usize = 10000;
    
    /// ファジー検索のスコア閾値
    pub const FUZZY_THRESHOLD: f64 = 0.3;
    
    /// プレビュー表示の最大行数
    pub const PREVIEW_MAX_LINES: usize = 10;
}

/// ファイル処理関連の定数
pub mod file_processing {
    /// 監視対象ファイル拡張子
    pub const WATCHED_EXTENSIONS: &[&str] = &[
        "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "cpp", "c", "h",
        "rb", "php", "cs", "swift", "kt", "scala", "clj", "hs", "ml", "elm"
    ];
    
    /// 処理対象ファイルの最大サイズ（MB）
    pub const MAX_FILE_SIZE_MB: u64 = 10;
    
    /// デバウンス時間（ファイル変更検出）
    pub const FILE_CHANGE_DEBOUNCE_MS: u64 = 150;
}

/// パフォーマンス関連の定数
pub mod performance {
    /// 並列処理の最大スレッド数
    pub const MAX_PARALLEL_THREADS: usize = 8;
    
    /// メモリ使用量の警告閾値（MB）
    pub const MEMORY_WARNING_MB: usize = 500;
    
    /// キャッシュエントリの最大数
    pub const MAX_CACHE_ENTRIES: usize = 10000;
    
    /// バックグラウンド処理のバッチサイズ
    pub const BACKGROUND_BATCH_SIZE: usize = 100;
}