use crate::types::{SearchResult, DisplayInfo, FormattedResult, ColorInfo, Color};
use super::traits::ResultFormatter;
use super::utils::{get_relative_path, color_to_ansi};

/// CLI用フォーマッター（折りたたみなし、フルパス表示）
pub struct CliFormatter {
    project_root: std::path::PathBuf,
    enable_colors: bool,
}

impl CliFormatter {
    pub fn new(project_root: std::path::PathBuf) -> Self {
        Self {
            project_root,
            enable_colors: false, // CLI用は色なし
        }
    }
}

impl ResultFormatter for CliFormatter {
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
            DisplayInfo::File { file_name: _ } => {
                let relative_path = get_relative_path(&result.file_path, &self.project_root);
                
                FormattedResult {
                    left_part: format!("📄 {}", relative_path),
                    right_part: String::new(),
                    color_info: ColorInfo {
                        path_color: Color::Blue,
                        location_color: Color::Gray,
                        content_color: Color::Cyan,
                        highlight_color: Color::Yellow,
                    },
                }
            }
            DisplayInfo::Regex { line_content, matched_text: _, match_start: _, match_end: _ } => {
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