use crate::types::{SearchResult, DisplayInfo, FormattedResult, ColorInfo, Color};
use super::traits::ResultFormatter;
use super::utils::{get_relative_path, color_to_ansi};

/// Symbol Search用 TTY形式フォーマッター（ファイル名ヘッダー + シンボル表示）
pub struct SymbolHeadingFormatter {
    project_root: std::path::PathBuf,
    enable_colors: bool,
}

/// Symbol Search用 Pipe形式フォーマッター（シンボル:ファイル名:行番号）
pub struct SymbolInlineFormatter {
    project_root: std::path::PathBuf,
    enable_colors: bool,
}

impl SymbolHeadingFormatter {
    pub fn new(project_root: std::path::PathBuf) -> Self {
        Self {
            project_root,
            enable_colors: true, // TTY形式では常に色有効
        }
    }
}

impl ResultFormatter for SymbolHeadingFormatter {
    fn format_result(&self, result: &SearchResult) -> FormattedResult {
        match &result.display_info {
            DisplayInfo::Symbol { name, symbol_type } => {
                let relative_path = get_relative_path(&result.file_path, &self.project_root);
                let symbol_display = format!("{}{}", symbol_type.icon(), name);
                
                FormattedResult {
                    left_part: symbol_display,
                    right_part: relative_path, // ファイル名ヘッダー用
                    color_info: ColorInfo {
                        path_color: Color::Blue,
                        location_color: Color::Gray,
                        content_color: Color::Green,
                        highlight_color: Color::Yellow,
                    },
                }
            }
            _ => panic!("SymbolHeadingFormatter should only handle Symbol searches"),
        }
    }

    fn to_colored_string(&self, formatted: &FormattedResult) -> String {
        if !self.enable_colors {
            return formatted.left_part.clone();
        }

        format!(
            "{}{}{}",
            color_to_ansi(&formatted.color_info.content_color),
            formatted.left_part,
            color_to_ansi(&Color::Reset),
        )
    }
}

impl SymbolInlineFormatter {
    pub fn new(project_root: std::path::PathBuf) -> Self {
        Self {
            project_root,
            enable_colors: false, // Pipe形式では色無効
        }
    }
}

impl ResultFormatter for SymbolInlineFormatter {
    fn format_result(&self, result: &SearchResult) -> FormattedResult {
        match &result.display_info {
            DisplayInfo::Symbol { name, symbol_type } => {
                let relative_path = get_relative_path(&result.file_path, &self.project_root);
                let symbol_display = format!("{}{}", symbol_type.icon(), name);
                
                FormattedResult {
                    left_part: format!("{}:{}:{}", symbol_display, relative_path, result.line),
                    right_part: String::new(),
                    color_info: ColorInfo {
                        path_color: Color::Blue,
                        location_color: Color::Gray,
                        content_color: Color::Green,
                        highlight_color: Color::Yellow,
                    },
                }
            }
            _ => panic!("SymbolInlineFormatter should only handle Symbol searches"),
        }
    }

    fn to_colored_string(&self, formatted: &FormattedResult) -> String {
        if !self.enable_colors {
            return formatted.left_part.clone();
        }

        format!(
            "{}{}{}",
            color_to_ansi(&formatted.color_info.content_color),
            formatted.left_part,
            color_to_ansi(&Color::Reset),
        )
    }
}