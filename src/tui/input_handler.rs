//! 入力処理ハンドラー
//! 
//! キーボードとマウスイベントの処理を分離

use crate::types::SearchMode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use super::text_editing::EditableText;

/// キーボード入力の処理結果
#[derive(Debug, Clone)]
pub enum InputResult {
    /// 処理完了、継続
    Continue,
    /// アプリケーション終了
    Quit,
    /// 検索モード変更
    ModeChanged(SearchMode),
    /// ヘルプ表示切り替え
    ToggleHelp,
    /// 結果選択・コピー
    SelectResult,
    /// 検索クエリ更新
    QueryUpdated,
    /// 結果リストナビゲーション
    Navigate(NavigationAction),
    /// 処理されなかった入力
    Unhandled,
}

/// ナビゲーション操作
#[derive(Debug, Clone)]
pub enum NavigationAction {
    Up,
    Down,
    PageUp,
    PageDown,
    HalfPageUp,
    HalfPageDown,
    Home,
    End,
}

/// 入力ハンドラー
pub struct InputHandler;

impl InputHandler {
    /// キーボードイベントを処理
    pub fn handle_key<T: EditableText>(
        state: &mut T,
        key: KeyEvent,
        current_mode: SearchMode,
        help_visible: bool,
        result_count: usize,
    ) -> InputResult {
        match (key.code, key.modifiers) {
            // 終了操作
            (KeyCode::Esc, _) => {
                if help_visible {
                    InputResult::ToggleHelp
                } else {
                    InputResult::Quit
                }
            }
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => InputResult::Quit,
            (KeyCode::Char('q'), KeyModifiers::NONE) if help_visible => InputResult::ToggleHelp,
            
            // ヘルプ表示
            (KeyCode::Char('?'), _) => InputResult::ToggleHelp,
            (KeyCode::F(1), _) => InputResult::ToggleHelp,
            
            // 結果選択
            (KeyCode::Enter, _) => InputResult::SelectResult,
            
            // 検索モード切り替え
            (KeyCode::Tab, KeyModifiers::NONE) => {
                let next_mode = Self::cycle_search_mode(current_mode);
                InputResult::ModeChanged(next_mode)
            }
            (KeyCode::BackTab, _) => {
                let prev_mode = Self::cycle_search_mode_reverse(current_mode);
                InputResult::ModeChanged(prev_mode)
            }
            
            // テキスト編集（ヘルプ表示中はスキップ）
            _ if !help_visible => Self::handle_text_editing(state, key, result_count),
            
            // その他は無視
            _ => InputResult::Unhandled,
        }
    }
    
    /// テキスト編集キーの処理
    fn handle_text_editing<T: EditableText>(
        state: &mut T,
        key: KeyEvent,
        result_count: usize,
    ) -> InputResult {
        match (key.code, key.modifiers) {
            // 文字入力
            (KeyCode::Char(ch), KeyModifiers::NONE) | 
            (KeyCode::Char(ch), KeyModifiers::SHIFT) => {
                state.insert_char(ch);
                InputResult::QueryUpdated
            }
            
            // 削除操作
            (KeyCode::Backspace, _) | 
            (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
                state.delete_char_backward();
                InputResult::QueryUpdated
            }
            (KeyCode::Delete, _) => {
                state.delete_char_forward();
                InputResult::QueryUpdated
            }
            
            // カーソル移動
            (KeyCode::Left, KeyModifiers::NONE) | 
            (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                let current_pos = state.cursor_position();
                if current_pos > 0 {
                    state.set_text_and_cursor(state.text().to_string(), current_pos - 1);
                }
                InputResult::Continue
            }
            (KeyCode::Right, KeyModifiers::NONE) | 
            (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                let current_pos = state.cursor_position();
                let max_pos = state.text().chars().count();
                if current_pos < max_pos {
                    state.set_text_and_cursor(state.text().to_string(), current_pos + 1);
                }
                InputResult::Continue
            }
            
            // 行頭・行末移動
            (KeyCode::Home, _) | 
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                state.move_cursor_to_beginning();
                InputResult::Continue
            }
            (KeyCode::End, _) | 
            (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                state.move_cursor_to_end();
                InputResult::Continue
            }
            
            // 単語移動
            (KeyCode::Left, KeyModifiers::CONTROL) => {
                state.move_cursor_word_backward();
                InputResult::Continue
            }
            (KeyCode::Right, KeyModifiers::CONTROL) => {
                state.move_cursor_word_forward();
                InputResult::Continue
            }
            
            // 行操作
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                state.kill_line();
                InputResult::QueryUpdated
            }
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                state.clear_line();
                InputResult::QueryUpdated
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                // 半ページ上スクロール専用
                if result_count > 0 {
                    InputResult::Navigate(NavigationAction::HalfPageUp)
                } else {
                    InputResult::Continue
                }
            }
            
            // 結果リストナビゲーション
            (KeyCode::Up, _) | 
            (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                if result_count > 0 {
                    InputResult::Navigate(NavigationAction::Up)
                } else {
                    InputResult::Continue
                }
            }
            (KeyCode::Down, _) | 
            (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                if result_count > 0 {
                    InputResult::Navigate(NavigationAction::Down)
                } else {
                    InputResult::Continue
                }
            }
            (KeyCode::PageUp, _) => {
                if result_count > 0 {
                    InputResult::Navigate(NavigationAction::PageUp)
                } else {
                    InputResult::Continue
                }
            }
            (KeyCode::PageDown, _) => {
                if result_count > 0 {
                    InputResult::Navigate(NavigationAction::PageDown)
                } else {
                    InputResult::Continue
                }
            }
            
            // 半ページスクロール（Ctrl-d）
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                if result_count > 0 {
                    InputResult::Navigate(NavigationAction::HalfPageDown)
                } else {
                    InputResult::Continue
                }
            }
            
            // その他は無視
            _ => InputResult::Unhandled,
        }
    }
    
    /// マウスイベントを処理
    pub fn handle_mouse(_event: MouseEvent) -> InputResult {
        // 将来的にマウス操作を実装
        InputResult::Unhandled
    }
    
    /// 検索モードを次に進める
    fn cycle_search_mode(current: SearchMode) -> SearchMode {
        match current {
            SearchMode::Content => SearchMode::Symbol,
            SearchMode::Symbol => SearchMode::File,
            SearchMode::File => SearchMode::Regex,
            SearchMode::Regex => SearchMode::Content,
        }
    }
    
    /// 検索モードを前に戻す
    fn cycle_search_mode_reverse(current: SearchMode) -> SearchMode {
        match current {
            SearchMode::Content => SearchMode::Regex,
            SearchMode::Symbol => SearchMode::Content,
            SearchMode::File => SearchMode::Symbol,
            SearchMode::Regex => SearchMode::File,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct TestEditableText {
        text: String,
        cursor: usize,
    }
    
    impl EditableText for TestEditableText {
        fn text(&self) -> &str { &self.text }
        fn cursor_position(&self) -> usize { self.cursor }
        fn set_text_and_cursor(&mut self, text: String, cursor: usize) {
            self.text = text;
            self.cursor = cursor;
        }
    }
    
    #[test]
    fn test_mode_cycling() {
        assert_eq!(
            InputHandler::cycle_search_mode(SearchMode::Content),
            SearchMode::Symbol
        );
        assert_eq!(
            InputHandler::cycle_search_mode_reverse(SearchMode::Symbol),
            SearchMode::Content
        );
    }
    
    #[test]
    fn test_text_editing() {
        let mut text = TestEditableText {
            text: "hello".to_string(),
            cursor: 2,
        };
        
        let result = InputHandler::handle_text_editing(
            &mut text,
            KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE),
            0
        );
        
        assert_eq!(text.text, "heXllo");
        assert_eq!(text.cursor, 3);
        assert!(matches!(result, InputResult::QueryUpdated));
    }
}