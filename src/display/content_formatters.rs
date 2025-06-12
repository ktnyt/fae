use crate::types::{SearchResult, DisplayInfo, FormattedResult, ColorInfo, Color};
use super::traits::ResultFormatter;
use super::utils::{get_relative_path, color_to_ansi};

/// Content Search用 TTY形式フォーマッター（ファイル名ヘッダー + 行番号:内容）
pub struct ContentHeadingFormatter {
    project_root: std::path::PathBuf,
    enable_colors: bool,
}

/// Content Search用 Pipe形式フォーマッター（ファイル名:行番号:内容）
pub struct ContentInlineFormatter {
    project_root: std::path::PathBuf,
    enable_colors: bool,
}

impl ContentHeadingFormatter {
    pub fn new(project_root: std::path::PathBuf) -> Self {
        Self {
            project_root,
            enable_colors: true, // TTY形式では常に色有効
        }
    }
}

impl ResultFormatter for ContentHeadingFormatter {
    fn format_result(&self, result: &SearchResult) -> FormattedResult {
        match &result.display_info {
            DisplayInfo::Content { line_content, match_start: _, match_end: _ } => {
                let relative_path = get_relative_path(&result.file_path, &self.project_root);
                let content = line_content.replace('\t', "    ").trim().to_string();
                
                FormattedResult {
                    left_part: format!("{}:{}", result.line, content),
                    right_part: relative_path, // ファイル名ヘッダー用
                    color_info: ColorInfo {
                        path_color: Color::Blue,
                        location_color: Color::Gray,
                        content_color: Color::White,
                        highlight_color: Color::Yellow,
                    },
                }
            }
            _ => panic!("ContentHeadingFormatter should only handle Content searches"),
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

impl ContentInlineFormatter {
    pub fn new(project_root: std::path::PathBuf) -> Self {
        Self {
            project_root,
            enable_colors: false, // Pipe形式では色無効
        }
    }
}

impl ResultFormatter for ContentInlineFormatter {
    fn format_result(&self, result: &SearchResult) -> FormattedResult {
        match &result.display_info {
            DisplayInfo::Content { line_content, match_start: _, match_end: _ } => {
                let relative_path = get_relative_path(&result.file_path, &self.project_root);
                let content = line_content.replace('\t', "    ").trim().to_string();
                
                FormattedResult {
                    left_part: format!("{}:{}:{}", relative_path, result.line, content),
                    right_part: String::new(),
                    color_info: ColorInfo {
                        path_color: Color::Blue,
                        location_color: Color::Gray,
                        content_color: Color::White,
                        highlight_color: Color::Yellow,
                    },
                }
            }
            _ => panic!("ContentInlineFormatter should only handle Content searches"),
        }
    }

    fn to_colored_string(&self, formatted: &FormattedResult) -> String {
        if !self.enable_colors {
            return formatted.left_part.clone();
        }

        format!(
            "{}{}{}",
            color_to_ansi(&formatted.color_info.path_color),
            formatted.left_part,
            color_to_ansi(&Color::Reset),
        )
    }
}