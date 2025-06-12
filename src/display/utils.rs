use crate::types::Color;
use std::path::Path;

/// カラーサポート検出
pub fn detect_color_support() -> bool {
    // NO_COLOR環境変数でカラー無効化
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }
    
    // FORCE_COLOR環境変数でカラー強制有効化（テスト用）
    if std::env::var("FORCE_COLOR").is_ok() {
        return true;
    }
    
    // 標準出力がTTYかつTERMが適切に設定されている場合のみカラー有効
    is_stdout_tty() && std::env::var("TERM").is_ok_and(|term| term != "dumb")
}

/// 標準出力がTTYかどうか判定
pub fn is_stdout_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Color enum を ANSI エスケープシーケンスに変換
pub fn color_to_ansi(color: &Color) -> &'static str {
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

/// ターミナル幅を検出
pub fn detect_terminal_width() -> usize {
    // crossterm を使用してターミナルサイズを取得
    if let Ok((width, _)) = crossterm::terminal::size() {
        width as usize
    } else {
        80 // デフォルト幅
    }
}

/// 相対パスを取得（基本版）
pub fn get_relative_path(absolute_path: &Path, project_root: &Path) -> String {
    absolute_path
        .strip_prefix(project_root)
        .unwrap_or(absolute_path)
        .to_string_lossy()
        .to_string()
}

/// パスを省略（先頭と末尾を残す）
pub fn truncate_path(path: &str) -> String {
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
pub fn create_context_preview(
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