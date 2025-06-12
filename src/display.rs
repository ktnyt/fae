use crate::types::{SearchResult, DisplayInfo, FormattedResult, ColorInfo, Color};
use std::path::Path;

/// 検索結果フォーマッターのトレイト
pub trait ResultFormatter {
    /// 検索結果をフォーマット
    fn format_result(&self, result: &SearchResult) -> FormattedResult;
    
    /// フォーマット済み結果を文字列に変換（色付き）
    fn to_colored_string(&self, formatted: &FormattedResult) -> String;
}

/// 検索結果の表示フォーマッター
pub struct DisplayFormatter {
    /// 現在のターミナル幅
    terminal_width: usize,
    /// 色分けを有効にするか
    enable_colors: bool,
    /// プロジェクトルート（相対パス計算用）
    project_root: std::path::PathBuf,
    /// 長いテキストを折りたたむか（CLI用はfalse）
    enable_truncation: bool,
}

impl DisplayFormatter {
    /// 新しいフォーマッターを作成（TUI用、折りたたみ有効）
    pub fn new(project_root: std::path::PathBuf) -> Self {
        Self {
            terminal_width: Self::detect_terminal_width(),
            enable_colors: Self::detect_color_support(),
            project_root,
            enable_truncation: true,
        }
    }

    /// CLI用フォーマッターを作成（折りたたみ無効）
    pub fn new_for_cli(project_root: std::path::PathBuf) -> Self {
        Self {
            terminal_width: Self::detect_terminal_width(),
            enable_colors: Self::detect_color_support(),
            project_root,
            enable_truncation: false,
        }
    }

    /// 検索結果をフォーマット
    pub fn format_result(&self, result: &SearchResult) -> FormattedResult {
        match &result.display_info {
            DisplayInfo::Content { line_content, match_start, match_end } => {
                self.format_content_result(result, line_content, *match_start, *match_end)
            }
            DisplayInfo::Symbol { name, symbol_type } => {
                self.format_symbol_result(result, name, symbol_type)
            }
            DisplayInfo::File { file_name } => {
                self.format_file_result(result, file_name)
            }
            DisplayInfo::Regex { line_content, matched_text: _, match_start, match_end } => {
                self.format_content_result(result, line_content, *match_start, *match_end)
            }
        }
    }

    /// コンテンツ/正規表現検索結果をフォーマット
    fn format_content_result(
        &self,
        result: &SearchResult,
        line_content: &str,
        match_start: usize,
        match_end: usize,
    ) -> FormattedResult {
        let relative_path = self.get_relative_path(&result.file_path);
        let location = format!("{}:{}:{}", relative_path, result.line, result.column);
        
        // CLI用は折りたたみなし、TUI用は幅制限
        let preview = if self.enable_truncation {
            let available_width = self.terminal_width.saturating_sub(location.len() + 4); // マージン考慮
            self.create_context_preview(line_content, match_start, match_end, available_width)
        } else {
            // CLI用: 全行を表示、タブ文字のみ正規化
            line_content.replace('\t', "    ").trim().to_string()
        };
        
        FormattedResult {
            left_part: location,
            right_part: preview,
            color_info: ColorInfo {
                path_color: Color::Blue,
                location_color: Color::Gray,
                content_color: Color::White,
                highlight_color: Color::Yellow,
            },
        }
    }

    /// シンボル検索結果をフォーマット
    fn format_symbol_result(
        &self,
        result: &SearchResult,
        name: &str,
        symbol_type: &crate::types::SymbolType,
    ) -> FormattedResult {
        let relative_path = self.get_relative_path(&result.file_path);
        let location = format!("{}:{}", relative_path, result.line);
        let symbol_display = format!("{} {}", symbol_type.icon(), name);
        
        FormattedResult {
            left_part: symbol_display,
            right_part: location,
            color_info: ColorInfo {
                path_color: Color::Blue,
                location_color: Color::Gray,
                content_color: Color::Green,
                highlight_color: Color::Yellow,
            },
        }
    }

    /// ファイル検索結果をフォーマット
    fn format_file_result(&self, result: &SearchResult, file_name: &str) -> FormattedResult {
        let relative_path = self.get_relative_path(&result.file_path);
        let parent_dir = Path::new(&relative_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "./".to_string());

        FormattedResult {
            left_part: format!("📄 {}", file_name),
            right_part: parent_dir,
            color_info: ColorInfo {
                path_color: Color::Blue,
                location_color: Color::Gray,
                content_color: Color::Cyan,
                highlight_color: Color::Yellow,
            },
        }
    }

    /// 相対パスを取得（パス省略機能付き）
    fn get_relative_path(&self, absolute_path: &Path) -> String {
        // 相対パス計算
        let relative_path = absolute_path
            .strip_prefix(&self.project_root)
            .unwrap_or(absolute_path)
            .to_string_lossy()
            .to_string();

        // CLI用は省略なし、TUI用は省略
        if self.enable_truncation {
            self.truncate_path(&relative_path)
        } else {
            relative_path
        }
    }

    /// パスを省略（先頭と末尾を残す）
    fn truncate_path(&self, path: &str) -> String {
        const MAX_PATH_LENGTH: usize = 50;
        
        if path.len() <= MAX_PATH_LENGTH {
            return path.to_string();
        }

        // パス要素に分割
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() <= 2 {
            return path.to_string();
        }

        // 先頭と末尾を保持して中間を省略
        let first = parts[0];
        let last = parts.last().unwrap();
        
        // 先頭 + "..." + 末尾の長さを計算
        let abbreviated = if first.is_empty() {
            format!(".../{}", last)
        } else {
            format!("{}/.../{}", first, last)
        };

        if abbreviated.len() < path.len() {
            abbreviated
        } else {
            path.to_string()
        }
    }

    /// ヒット箇所を中心としたプレビューを作成
    fn create_context_preview(
        &self,
        line_content: &str,
        match_start: usize,
        match_end: usize,
        max_width: usize,
    ) -> String {
        if max_width < 10 {
            return "...".to_string();
        }

        // マッチ部分の長さ
        let match_length = match_end.saturating_sub(match_start);
        
        // マッチ部分が表示幅より長い場合
        if match_length >= max_width {
            let safe_truncated = line_content.chars()
                .skip(match_start)
                .take(max_width - 3)
                .collect::<String>();
            return format!("{}...", safe_truncated);
        }

        // 前後のコンテキストを計算
        let remaining_width = max_width - match_length;
        let before_width = remaining_width / 2;
        let after_width = remaining_width - before_width;

        // 実際の開始・終了位置を計算
        let preview_start = match_start.saturating_sub(before_width);
        let preview_end = std::cmp::min(
            line_content.len(),
            match_end + after_width,
        );

        // プレビュー文字列を構築
        let mut preview = String::new();
        
        // 開始部分が省略されている場合
        if preview_start > 0 {
            preview.push_str("...");
        }
        
        // 実際のコンテンツ（UTF-8安全に取得）
        let safe_content = line_content.chars()
            .skip(preview_start)
            .take(preview_end - preview_start)
            .collect::<String>();
        preview.push_str(&safe_content);
        
        // 終了部分が省略されている場合
        if preview_end < line_content.len() {
            preview.push_str("...");
        }

        // 空白文字を正規化
        preview.replace('\t', "    ").trim().to_string()
    }

    /// フォーマット済み結果を文字列に変換（色付き）
    pub fn to_colored_string(&self, formatted: &FormattedResult) -> String {
        if !self.enable_colors {
            return format!("{:<40} {}", formatted.left_part, formatted.right_part);
        }

        // ANSI カラーコードを適用
        format!(
            "{}{:<40}{} {}{}{}",
            self.color_to_ansi(&formatted.color_info.content_color),
            formatted.left_part,
            self.color_to_ansi(&Color::Reset),
            self.color_to_ansi(&formatted.color_info.path_color),
            formatted.right_part,
            self.color_to_ansi(&Color::Reset),
        )
    }

    /// ターミナル幅を検出
    fn detect_terminal_width() -> usize {
        // crossterm を使用してターミナルサイズを取得
        if let Ok((width, _)) = crossterm::terminal::size() {
            width as usize
        } else {
            80 // デフォルト幅
        }
    }

    /// カラーサポートを検出
    fn detect_color_support() -> bool {
        // 環境変数やターミナル種別から判定
        std::env::var("NO_COLOR").is_err() && 
        std::env::var("TERM").is_ok_and(|term| term != "dumb")
    }

    /// Color enum を ANSI エスケープシーケンスに変換
    fn color_to_ansi(&self, color: &Color) -> &'static str {
        match color {
            Color::Reset => "\x1b[0m",
            Color::Gray => "\x1b[90m",
            Color::Blue => "\x1b[34m",
            Color::Green => "\x1b[32m",
            Color::Yellow => "\x1b[33m",
            Color::Red => "\x1b[31m",
            Color::Cyan => "\x1b[36m",
            Color::White => "\x1b[37m",
        }
    }
}

/// CLI専用フォーマッター（折りたたみなし）
pub struct CliFormatter {
    project_root: std::path::PathBuf,
    enable_colors: bool,
}

impl CliFormatter {
    pub fn new(project_root: std::path::PathBuf) -> Self {
        Self {
            project_root,
            enable_colors: detect_color_support(),
        }
    }
    
    fn get_relative_path(&self, absolute_path: &Path) -> String {
        absolute_path
            .strip_prefix(&self.project_root)
            .unwrap_or(absolute_path)
            .to_string_lossy()
            .to_string()
    }
}

impl ResultFormatter for CliFormatter {
    fn format_result(&self, result: &SearchResult) -> FormattedResult {
        match &result.display_info {
            DisplayInfo::Content { line_content, match_start: _, match_end: _ } => {
                let relative_path = self.get_relative_path(&result.file_path);
                let location = format!("{}:{}:{}", relative_path, result.line, result.column);
                let content = line_content.replace('\t', "    ").trim().to_string();
                
                FormattedResult {
                    left_part: location,
                    right_part: content,
                    color_info: ColorInfo {
                        path_color: Color::Blue,
                        location_color: Color::Gray,
                        content_color: Color::White,
                        highlight_color: Color::Yellow,
                    },
                }
            }
            DisplayInfo::Symbol { name, symbol_type } => {
                let relative_path = self.get_relative_path(&result.file_path);
                let location = format!("{}:{}", relative_path, result.line);
                let symbol_display = format!("{} {}", symbol_type.icon(), name);
                
                FormattedResult {
                    left_part: symbol_display,
                    right_part: location,
                    color_info: ColorInfo {
                        path_color: Color::Blue,
                        location_color: Color::Gray,
                        content_color: Color::Green,
                        highlight_color: Color::Yellow,
                    },
                }
            }
            DisplayInfo::File { file_name } => {
                let relative_path = self.get_relative_path(&result.file_path);
                let parent_dir = Path::new(&relative_path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "./".to_string());

                FormattedResult {
                    left_part: format!("📄 {}", file_name),
                    right_part: parent_dir,
                    color_info: ColorInfo {
                        path_color: Color::Blue,
                        location_color: Color::Gray,
                        content_color: Color::Cyan,
                        highlight_color: Color::Yellow,
                    },
                }
            }
            DisplayInfo::Regex { line_content, matched_text: _, match_start: _, match_end: _ } => {
                let relative_path = self.get_relative_path(&result.file_path);
                let location = format!("{}:{}:{}", relative_path, result.line, result.column);
                let content = line_content.replace('\t', "    ").trim().to_string();
                
                FormattedResult {
                    left_part: location,
                    right_part: content,
                    color_info: ColorInfo {
                        path_color: Color::Blue,
                        location_color: Color::Gray,
                        content_color: Color::White,
                        highlight_color: Color::Yellow,
                    },
                }
            }
        }
    }

    fn to_colored_string(&self, formatted: &FormattedResult) -> String {
        if !self.enable_colors {
            return format!("{:<50} {}", formatted.left_part, formatted.right_part);
        }

        // ANSI カラーコードを適用
        format!(
            "{}{:<50}{} {}{}{}",
            color_to_ansi(&formatted.color_info.content_color),
            formatted.left_part,
            color_to_ansi(&Color::Reset),
            color_to_ansi(&formatted.color_info.path_color),
            formatted.right_part,
            color_to_ansi(&Color::Reset),
        )
    }
}

/// TUI専用フォーマッター（折りたたみあり）
pub struct TuiFormatter {
    formatter: DisplayFormatter,
}

impl TuiFormatter {
    pub fn new(project_root: std::path::PathBuf) -> Self {
        Self {
            formatter: DisplayFormatter::new(project_root),
        }
    }
}

impl ResultFormatter for TuiFormatter {
    fn format_result(&self, result: &SearchResult) -> FormattedResult {
        self.formatter.format_result(result)
    }

    fn to_colored_string(&self, formatted: &FormattedResult) -> String {
        self.formatter.to_colored_string(formatted)
    }
}

/// カラーサポート検出
fn detect_color_support() -> bool {
    std::env::var("NO_COLOR").is_err() && 
    std::env::var("TERM").is_ok_and(|term| term != "dumb")
}

/// Color enum を ANSI エスケープシーケンスに変換
fn color_to_ansi(color: &Color) -> &'static str {
    match color {
        Color::Reset => "\x1b[0m",
        Color::Gray => "\x1b[90m",
        Color::Blue => "\x1b[34m",
        Color::Green => "\x1b[32m",
        Color::Yellow => "\x1b[33m",
        Color::Red => "\x1b[31m",
        Color::Cyan => "\x1b[36m",
        Color::White => "\x1b[37m",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SearchResult, DisplayInfo, SymbolType};
    use std::path::PathBuf;

    fn create_test_formatter() -> DisplayFormatter {
        DisplayFormatter {
            terminal_width: 100,
            enable_colors: false,
            project_root: PathBuf::from("/test/project"),
            enable_truncation: true,
        }
    }

    #[test]
    fn test_path_truncation() {
        let formatter = create_test_formatter();
        
        // 短いパス（省略なし）
        let short_path = "src/main.rs";
        assert_eq!(formatter.truncate_path(short_path), "src/main.rs");
        
        // 長いパス（省略あり）
        let long_path = "src/very/deep/nested/directory/structure/with/many/levels/file.rs";
        let truncated = formatter.truncate_path(long_path);
        assert!(truncated.len() < long_path.len());
        assert!(truncated.contains("..."));
        assert!(truncated.ends_with("file.rs"));
    }

    #[test]
    fn test_context_preview() {
        let formatter = create_test_formatter();
        let line = "    const result = calculateSomething(input);";
        
        // "calculateSomething" がマッチした場合
        let match_start = 19;
        let match_end = 35;
        
        let preview = formatter.create_context_preview(line, match_start, match_end, 50);
        
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