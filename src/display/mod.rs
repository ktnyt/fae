/// フォーマッターモジュール

mod traits;
mod display_formatter;
mod content_formatters;
mod symbol_formatters;
mod cli_formatter;
mod tui_formatter;
mod utils;

// パブリックAPIをエクスポート
pub use traits::ResultFormatter;
pub use display_formatter::DisplayFormatter;
pub use content_formatters::{ContentHeadingFormatter, ContentInlineFormatter};
pub use symbol_formatters::{SymbolHeadingFormatter, SymbolInlineFormatter};
pub use cli_formatter::CliFormatter;
pub use tui_formatter::TuiFormatter;

// テスト用に一部の関数もエクスポート
pub use utils::{truncate_path, create_context_preview};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SearchResult, DisplayInfo, SymbolType};
    use std::path::PathBuf;

    fn create_test_formatter() -> DisplayFormatter {
        DisplayFormatter::new_for_cli(PathBuf::from("/test/project"))
    }

    #[test]
    fn test_path_truncation() {
        // 短いパス（省略なし）
        let short_path = "src/main.rs";
        assert_eq!(truncate_path(short_path), "src/main.rs");
        
        // 長いパス（省略あり）
        let long_path = "src/very/deep/nested/directory/structure/with/many/levels/file.rs";
        let truncated = truncate_path(long_path);
        assert!(truncated.len() < long_path.len());
        assert!(truncated.contains("..."));
        assert!(truncated.ends_with("file.rs"));
    }

    #[test]
    fn test_context_preview() {
        let line = "    const result = calculateSomething(input);";
        
        // "calculateSomething" がマッチした場合
        let match_start = 19;
        let match_end = 35;
        
        let preview = create_context_preview(line, match_start, match_end, 50);
        
        assert!(preview.contains("calculateSomething"));
        assert!(preview.len() <= 50);
    }

    #[test]
    fn test_symbol_formatting() {
        let formatter = create_test_formatter();
        
        let result = SearchResult {
            file_path: PathBuf::from("/test/project/src/main.rs"),
            line: 42,
            column: 8,
            display_info: DisplayInfo::Symbol {
                name: "test_function".to_string(),
                symbol_type: SymbolType::Function,
            },
            score: 1.0,
        };

        let formatted = formatter.format_result(&result);
        assert!(formatted.left_part.contains("🔧"));
        assert!(formatted.left_part.contains("test_function"));
        assert!(formatted.right_part.contains("src/main.rs:42"));
    }
}