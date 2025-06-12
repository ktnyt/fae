use crate::types::{SearchResult, FormattedResult};

/// 検索結果フォーマッターのトレイト
pub trait ResultFormatter {
    /// 検索結果をフォーマット
    fn format_result(&self, result: &SearchResult) -> FormattedResult;
    
    /// フォーマット済み結果を文字列に変換（色付き）
    fn to_colored_string(&self, formatted: &FormattedResult) -> String;
}