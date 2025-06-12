use crate::types::{SearchResult, DisplayInfo, FormattedResult, ColorInfo, Color};
use super::traits::ResultFormatter;
use super::utils::{color_to_ansi, truncate_path, create_context_preview, detect_color_support, detect_terminal_width};
use std::path::Path;

/// TUI用フォーマッター（折りたたみあり、パス省略あり）
pub struct TuiFormatter {
    terminal_width: usize,
    enable_colors: bool,
    project_root: std::path::PathBuf,
}

impl TuiFormatter {
    pub fn new(project_root: std::path::PathBuf) -> Self {
        Self {
            terminal_width: detect_terminal_width(),
            enable_colors: detect_color_support(),
            project_root,
        }
    }

    /// 相対パスを取得（パス省略機能付き）
    fn get_relative_path(&self, absolute_path: &Path) -> String {
        let relative_path = absolute_path
            .strip_prefix(&self.project_root)
            .unwrap_or(absolute_path)
            .to_string_lossy()
            .to_string();

        truncate_path(&relative_path)
    }
}

impl ResultFormatter for TuiFormatter {
    fn format_result(&self, result: &SearchResult) -> FormattedResult {
        match &result.display_info {
            DisplayInfo::Content { line_content, match_start, match_end } => {
                let relative_path = self.get_relative_path(&result.file_path);
                let location = format!("{}:{}:{}", relative_path, result.line, result.column);
                
                // TUI用は幅制限
                let available_width = self.terminal_width.saturating_sub(location.len() + 4); // マージン考慮
                let preview = create_context_preview(line_content, *match_start, *match_end, available_width);
                
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
            DisplayInfo::Regex { line_content, matched_text: _, match_start, match_end } => {
                let relative_path = self.get_relative_path(&result.file_path);
                let location = format!("{}:{}:{}", relative_path, result.line, result.column);
                
                // TUI用は幅制限
                let available_width = self.terminal_width.saturating_sub(location.len() + 4); // マージン考慮
                let preview = create_context_preview(line_content, *match_start, *match_end, available_width);
                
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
        }
    }

    fn to_colored_string(&self, formatted: &FormattedResult) -> String {
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