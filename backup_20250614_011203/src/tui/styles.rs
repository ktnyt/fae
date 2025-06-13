//! TUIスタイル定義
//! 
//! 一貫したスタイル設定とテーマ管理

use ratatui::style::{Color, Modifier, Style};

/// TUIの全スタイル定義
pub struct TuiStyles {
    // 検索モード関連
    pub mode_content: Style,
    pub mode_symbol: Style,
    pub mode_file: Style,
    pub mode_regex: Style,
    
    // プレフィックス（#, >, /）
    pub prefix_symbol: Style,
    pub prefix_file: Style,
    pub prefix_regex: Style,
    
    // UI要素
    pub selection_bg: Style,
    pub input_cursor: Style,
    pub status_bar: Style,
    pub help_text: Style,
    pub error_text: Style,
    pub highlight: Style,
    pub loading: Style,
    
    // 結果表示
    pub result_selected: Style,
    pub result_normal: Style,
    pub line_number: Style,
    pub results_title: Style,
    pub results_empty: Style,
    
    // ボーダーとフレーム
    pub border_normal: Style,
    pub border_focused: Style,
}

impl Default for TuiStyles {
    fn default() -> Self {
        Self {
            // 検索モード（色付きで太字）
            mode_content: Style::default().fg(Color::White),
            mode_symbol: Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            mode_file: Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            mode_regex: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            
            // プレフィックス
            prefix_symbol: Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            prefix_file: Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            prefix_regex: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            
            // UI要素
            selection_bg: Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD),
            input_cursor: Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK),
            status_bar: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            help_text: Style::default().fg(Color::Gray),
            error_text: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            highlight: Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            loading: Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK),
            
            // 結果表示
            result_selected: Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD),
            result_normal: Style::default().fg(Color::White),
            line_number: Style::default().fg(Color::Gray),
            results_title: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            results_empty: Style::default().fg(Color::Gray),
            
            // ボーダーとフレーム
            border_normal: Style::default().fg(Color::Gray),
            border_focused: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        }
    }
}

impl TuiStyles {
    /// 検索モードに対応するスタイルを取得
    pub fn mode_style(&self, mode: &crate::types::SearchMode) -> Style {
        match mode {
            crate::types::SearchMode::Content => self.mode_content,
            crate::types::SearchMode::Symbol => self.mode_symbol,
            crate::types::SearchMode::File => self.mode_file,
            crate::types::SearchMode::Regex => self.mode_regex,
        }
    }
    
    /// 検索モードに対応する色を取得
    pub fn mode_color(&self, mode: &crate::types::SearchMode) -> Color {
        match mode {
            crate::types::SearchMode::Content => Color::White,
            crate::types::SearchMode::Symbol => Color::Green,
            crate::types::SearchMode::File => Color::Blue,
            crate::types::SearchMode::Regex => Color::Red,
        }
    }
    
    /// 検索モードに対応するプレフィックス文字を取得
    pub fn mode_prefix(&self, mode: &crate::types::SearchMode) -> Option<&'static str> {
        match mode {
            crate::types::SearchMode::Content => None,
            crate::types::SearchMode::Symbol => Some("#"),
            crate::types::SearchMode::File => Some(">"),
            crate::types::SearchMode::Regex => Some("/"),
        }
    }
}

/// ダークテーマのスタイル設定
pub fn dark_theme() -> TuiStyles {
    TuiStyles::default()
}

/// ライトテーマのスタイル設定（将来拡張用）
#[allow(dead_code)]
pub fn light_theme() -> TuiStyles {
    TuiStyles {
        mode_content: Style::default().fg(Color::Black),
        mode_symbol: Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        mode_file: Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
        mode_regex: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        
        prefix_symbol: Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        prefix_file: Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
        prefix_regex: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        
        selection_bg: Style::default().bg(Color::LightBlue).fg(Color::Black).add_modifier(Modifier::BOLD),
        input_cursor: Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK),
        status_bar: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        help_text: Style::default().fg(Color::Gray),
        error_text: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        highlight: Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        loading: Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK),
        
        result_selected: Style::default().bg(Color::LightBlue).fg(Color::Black).add_modifier(Modifier::BOLD),
        result_normal: Style::default().fg(Color::Black),
        line_number: Style::default().fg(Color::Gray),
        results_title: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        results_empty: Style::default().fg(Color::Gray),
        
        border_normal: Style::default().fg(Color::Gray),
        border_focused: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    }
}