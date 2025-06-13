//! JSON-RPC Message Types for Workers
//! 
//! 設計ドキュメント(.claude/tuidesign.md)に基づくメッセージ型定義

use serde::{Serialize, Deserialize};

/// ユーザークエリリクエスト (TUI → Worker)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    /// 検索クエリ文字列
    pub query: String,
}

/// 検索マッチ結果 (Worker → TUI)  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatch {
    /// ファイル名
    pub filename: String,
    /// 行番号 (1ベース)
    pub line: u32,
    /// カラム番号 (1ベース)
    pub column: u32,
    /// マッチした行の内容
    pub content: String,
}

impl SearchMatch {
    pub fn new(filename: String, line: u32, column: u32, content: String) -> Self {
        Self {
            filename,
            line,
            column, 
            content,
        }
    }
}