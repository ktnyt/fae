use crate::types::{SearchResult, DisplayInfo, FormattedResult, ColorInfo, Color};
use std::path::Path;
use super::utils::{detect_color_support, detect_terminal_width, create_context_preview, truncate_path, color_to_ansi};

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
            terminal_width: detect_terminal_width(),
            enable_colors: detect_color_support(),
            project_root,
            enable_truncation: true,
        }
    }

    /// CLI用フォーマッターを作成（折りたたみ無効）
    pub fn new_for_cli(project_root: std::path::PathBuf) -> Self {
        Self {
            terminal_width: detect_terminal_width(),
            enable_colors: detect_color_support(),
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
            create_context_preview(line_content, match_start, match_end, available_width)
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
            truncate_path(&relative_path)
        } else {
            relative_path
        }
    }

    /// フォーマット済み結果を文字列に変換（色付き）
    pub fn to_colored_string(&self, formatted: &FormattedResult) -> String {
        if !self.enable_colors {
            return format!("{:<40} {}", formatted.left_part, formatted.right_part);
        }

        // ANSI カラーコードを適用
        format!(
            "{}{:<40}{} {}{}{}",
            color_to_ansi(&formatted.color_info.content_color),
            formatted.left_part,
            color_to_ansi(&Color::Reset),
            color_to_ansi(&formatted.color_info.path_color),
            formatted.right_part,
            color_to_ansi(&Color::Reset),
        )
    }
}