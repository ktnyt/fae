use super::traits::ResultFormatter;
use super::utils::apply_color;
use crate::types::{SearchResult, DisplayInfo, FormattedResult, ColorInfo, Color};
use std::path::PathBuf;

/// ファイル検索用ヘッダー形式フォーマッター（TTY用）
pub struct FileHeadingFormatter {
    _project_root: PathBuf,
}

/// ファイル検索用インライン形式フォーマッター（Pipe用）
pub struct FileInlineFormatter {
    _project_root: PathBuf,
}

impl FileHeadingFormatter {
    pub fn new(project_root: PathBuf) -> Self {
        Self { _project_root: project_root }
    }
}

impl FileInlineFormatter {
    pub fn new(project_root: PathBuf) -> Self {
        Self { _project_root: project_root }
    }
}

impl ResultFormatter for FileHeadingFormatter {
    fn format_result(&self, result: &SearchResult) -> FormattedResult {
        match &result.display_info {
            DisplayInfo::File { path, is_directory } => {
                let icon = if *is_directory { "📁" } else { "📄" };
                let path_str = path.to_string_lossy();
                let left_part = format!("{}  {}", icon, path_str);
                
                // ファイル検索では右側は空
                let right_part = String::new();
                
                // 色分け情報
                let color_info = ColorInfo {
                    path_color: if *is_directory { Color::Blue } else { Color::White },
                    location_color: Color::Gray,
                    content_color: Color::White,
                    highlight_color: Color::Yellow,
                };
                
                FormattedResult {
                    left_part,
                    right_part,
                    color_info,
                }
            }
            _ => FormattedResult {
                left_part: String::new(),
                right_part: String::new(),
                color_info: ColorInfo {
                    path_color: Color::White,
                    location_color: Color::Gray,
                    content_color: Color::White,
                    highlight_color: Color::Yellow,
                },
            },
        }
    }

    fn to_colored_string(&self, formatted: &FormattedResult) -> String {
        let is_directory = formatted.left_part.starts_with("📁");
        if is_directory {
            apply_color(&formatted.left_part, &Color::Blue, true)
        } else {
            formatted.left_part.clone()
        }
    }
}

impl ResultFormatter for FileInlineFormatter {
    fn format_result(&self, result: &SearchResult) -> FormattedResult {
        match &result.display_info {
            DisplayInfo::File { path, is_directory } => {
                let type_indicator = if *is_directory { "/" } else { "" };
                let path_str = path.to_string_lossy();
                let left_part = format!("{}{}", path_str, type_indicator);
                
                // ファイル検索では右側は空
                let right_part = String::new();
                
                // 色分け情報
                let color_info = ColorInfo {
                    path_color: if *is_directory { Color::Blue } else { Color::White },
                    location_color: Color::Gray,
                    content_color: Color::White,
                    highlight_color: Color::Yellow,
                };
                
                FormattedResult {
                    left_part,
                    right_part,
                    color_info,
                }
            }
            _ => FormattedResult {
                left_part: String::new(),
                right_part: String::new(),
                color_info: ColorInfo {
                    path_color: Color::White,
                    location_color: Color::Gray,
                    content_color: Color::White,
                    highlight_color: Color::Yellow,
                },
            },
        }
    }

    fn to_colored_string(&self, formatted: &FormattedResult) -> String {
        let is_directory = formatted.left_part.ends_with('/');
        if is_directory {
            apply_color(&formatted.left_part, &Color::Blue, true)
        } else {
            formatted.left_part.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_file_result(path: &str, is_directory: bool) -> SearchResult {
        SearchResult {
            file_path: PathBuf::from("/test/project").join(path),
            line: 1,
            column: 1,
            display_info: DisplayInfo::File {
                path: PathBuf::from(path),
                is_directory,
            },
            score: 1.0,
        }
    }

    #[test]
    fn test_file_heading_formatter() {
        let formatter = FileHeadingFormatter::new(PathBuf::from("/test/project"));
        
        // ファイルのフォーマット
        let file_result = create_test_file_result("src/main.rs", false);
        let formatted = formatter.format_result(&file_result);
        assert_eq!(formatted.left_part, "📄  src/main.rs");
        
        // ディレクトリのフォーマット
        let dir_result = create_test_file_result("src/utils", true);
        let formatted = formatter.format_result(&dir_result);
        assert_eq!(formatted.left_part, "📁  src/utils");
    }

    #[test]
    fn test_file_inline_formatter() {
        let formatter = FileInlineFormatter::new(PathBuf::from("/test/project"));
        
        // ファイルのフォーマット
        let file_result = create_test_file_result("src/main.rs", false);
        let formatted = formatter.format_result(&file_result);
        assert_eq!(formatted.left_part, "src/main.rs");
        
        // ディレクトリのフォーマット
        let dir_result = create_test_file_result("src/utils", true);
        let formatted = formatter.format_result(&dir_result);
        assert_eq!(formatted.left_part, "src/utils/");
    }

    #[test]
    fn test_file_formatter_coloring() {
        let heading_formatter = FileHeadingFormatter::new(PathBuf::from("/test"));
        let inline_formatter = FileInlineFormatter::new(PathBuf::from("/test"));
        
        // ディレクトリの色付けテスト
        let dir_result = create_test_file_result("docs", true);
        let heading_formatted = heading_formatter.format_result(&dir_result);
        let heading_colored = heading_formatter.to_colored_string(&heading_formatted);
        assert!(heading_colored.contains("\x1b[34m")); // 青色のANSIコード
        
        let inline_formatted = inline_formatter.format_result(&dir_result);
        let inline_colored = inline_formatter.to_colored_string(&inline_formatted);
        assert!(inline_colored.contains("\x1b[34m")); // 青色のANSIコード
    }
}