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
    
    /// マッチした部分をハイライトして返す
    fn highlight_content(&self, content: &str, match_start: usize, match_end: usize) -> String {
        // UTF-8安全な文字境界を確認
        let safe_start = self.find_char_boundary(content, match_start, true);
        let safe_end = self.find_char_boundary(content, match_end, false);
        
        if safe_start >= content.len() || safe_end > content.len() || safe_start >= safe_end {
            return content.to_string();
        }
        
        let before = &content[..safe_start];
        let matched = &content[safe_start..safe_end];
        let after = &content[safe_end..];
        
        format!(
            "{}{}{}{}{}{}{}",
            color_to_ansi(&Color::White),
            before,
            color_to_ansi(&Color::Yellow),
            matched,
            color_to_ansi(&Color::Reset),
            color_to_ansi(&Color::White),
            after
        )
    }
    
    /// UTF-8安全な文字境界を見つける
    fn find_char_boundary(&self, content: &str, pos: usize, search_backward: bool) -> usize {
        if pos >= content.len() {
            return content.len();
        }
        
        if content.is_char_boundary(pos) {
            return pos;
        }
        
        if search_backward {
            // 後方検索：文字境界まで戻る
            (0..=pos).rev().find(|&i| content.is_char_boundary(i)).unwrap_or(0)
        } else {
            // 前方検索：文字境界まで進む
            (pos..content.len()).find(|&i| content.is_char_boundary(i)).unwrap_or(content.len())
        }
    }
}

impl ResultFormatter for ContentHeadingFormatter {
    fn format_result(&self, result: &SearchResult) -> FormattedResult {
        match &result.display_info {
            DisplayInfo::Content { line_content, match_start, match_end } => {
                let relative_path = get_relative_path(&result.file_path, &self.project_root);
                
                // タブを空白に変換（位置はそのまま維持）
                let tab_replaced = line_content.replace('\t', "    ");
                
                // trimによる位置のずれを計算
                let trimmed_start = tab_replaced.len() - tab_replaced.trim_start().len();
                let content = tab_replaced.trim().to_string();
                
                // match位置をtrim後の位置に調整
                let adjusted_start = match_start.saturating_sub(trimmed_start);
                let adjusted_end = match_end.saturating_sub(trimmed_start);
                
                // ハイライト付きの内容を生成
                let highlighted_content = if self.enable_colors && adjusted_start < content.len() && adjusted_end <= content.len() && adjusted_start < adjusted_end {
                    self.highlight_content(&content, adjusted_start, adjusted_end)
                } else {
                    content.clone()
                };
                
                FormattedResult {
                    left_part: format!("{}:{}", result.line, highlighted_content),
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

        // ハイライトは既に適用済みなので、リセットだけ追加
        format!(
            "{}{}",
            formatted.left_part,
            color_to_ansi(&Color::Reset),
        )
    }
}

impl ContentInlineFormatter {
    pub fn new(project_root: std::path::PathBuf) -> Self {
        // TTYかどうかを自動検出
        use std::io::IsTerminal;
        let enable_colors = IsTerminal::is_terminal(&std::io::stdout());
        
        Self {
            project_root,
            enable_colors,
        }
    }
    
    /// マッチした部分をハイライトして返す
    fn highlight_content(&self, content: &str, match_start: usize, match_end: usize) -> String {
        // UTF-8安全な文字境界を確認
        let safe_start = self.find_char_boundary(content, match_start, true);
        let safe_end = self.find_char_boundary(content, match_end, false);
        
        if safe_start >= content.len() || safe_end > content.len() || safe_start >= safe_end {
            return content.to_string();
        }
        
        let before = &content[..safe_start];
        let matched = &content[safe_start..safe_end];
        let after = &content[safe_end..];
        
        format!(
            "{}{}{}{}{}{}{}",
            color_to_ansi(&Color::White),
            before,
            color_to_ansi(&Color::Yellow),
            matched,
            color_to_ansi(&Color::Reset),
            color_to_ansi(&Color::White),
            after
        )
    }
    
    /// UTF-8安全な文字境界を見つける
    fn find_char_boundary(&self, content: &str, pos: usize, search_backward: bool) -> usize {
        if pos >= content.len() {
            return content.len();
        }
        
        if content.is_char_boundary(pos) {
            return pos;
        }
        
        if search_backward {
            // 後方検索：文字境界まで戻る
            (0..=pos).rev().find(|&i| content.is_char_boundary(i)).unwrap_or(0)
        } else {
            // 前方検索：文字境界まで進む
            (pos..content.len()).find(|&i| content.is_char_boundary(i)).unwrap_or(content.len())
        }
    }
}

impl ResultFormatter for ContentInlineFormatter {
    fn format_result(&self, result: &SearchResult) -> FormattedResult {
        match &result.display_info {
            DisplayInfo::Content { line_content, match_start, match_end } => {
                let relative_path = get_relative_path(&result.file_path, &self.project_root);
                
                // タブを空白に変換（位置はそのまま維持）
                let tab_replaced = line_content.replace('\t', "    ");
                
                // trimによる位置のずれを計算
                let trimmed_start = tab_replaced.len() - tab_replaced.trim_start().len();
                let content = tab_replaced.trim().to_string();
                
                // match位置をtrim後の位置に調整
                let adjusted_start = match_start.saturating_sub(trimmed_start);
                let adjusted_end = match_end.saturating_sub(trimmed_start);
                
                // ハイライト付きの内容を生成
                let highlighted_content = if self.enable_colors && adjusted_start < content.len() && adjusted_end <= content.len() && adjusted_start < adjusted_end {
                    self.highlight_content(&content, adjusted_start, adjusted_end)
                } else {
                    content.clone()
                };
                
                FormattedResult {
                    left_part: format!("{}:{}:{}", relative_path, result.line, highlighted_content),
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

        // ハイライトは既に適用済みなので、リセットだけ追加
        format!(
            "{}{}",
            formatted.left_part,
            color_to_ansi(&Color::Reset),
        )
    }
}