//! テキスト編集機能
//! 
//! 文字列操作とカーソル管理の共通ロジック

/// テキスト編集のヘルパー関数群
pub struct TextEditor;

impl TextEditor {
    /// 指定位置に文字を挿入
    pub fn insert_char_at(text: &str, position: usize, ch: char) -> (String, usize) {
        let chars: Vec<char> = text.chars().collect();
        let mut new_chars = chars;
        new_chars.insert(position, ch);
        let new_text = new_chars.into_iter().collect();
        (new_text, position + 1)
    }
    
    /// 指定位置の文字を削除（前方削除）
    pub fn delete_char_forward_at(text: &str, position: usize) -> (String, usize) {
        let chars: Vec<char> = text.chars().collect();
        if position < chars.len() {
            let mut new_chars = chars;
            new_chars.remove(position);
            let new_text = new_chars.into_iter().collect();
            (new_text, position)
        } else {
            (text.to_string(), position)
        }
    }
    
    /// 指定位置の文字を削除（後方削除）
    pub fn delete_char_backward_at(text: &str, position: usize) -> (String, usize) {
        if position > 0 {
            let chars: Vec<char> = text.chars().collect();
            let mut new_chars = chars;
            new_chars.remove(position - 1);
            let new_text = new_chars.into_iter().collect();
            (new_text, position - 1)
        } else {
            (text.to_string(), position)
        }
    }
    
    /// カーソル位置から行末まで削除
    pub fn kill_line_at(text: &str, position: usize) -> (String, usize) {
        let chars: Vec<char> = text.chars().collect();
        let new_chars: Vec<char> = chars.into_iter().take(position).collect();
        let new_text = new_chars.into_iter().collect();
        (new_text, position)
    }
    
    /// 行全体をクリア
    pub fn clear_line() -> (String, usize) {
        (String::new(), 0)
    }
    
    /// カーソル位置を文字境界に調整
    pub fn clamp_cursor_position(text: &str, position: usize) -> usize {
        let char_count = text.chars().count();
        position.min(char_count)
    }
    
    /// 文字列の指定位置から単語の境界を検索
    pub fn find_word_boundary_forward(text: &str, position: usize) -> usize {
        let chars: Vec<char> = text.chars().collect();
        let mut pos = position;
        
        // 現在位置が単語文字でない場合、単語文字まで移動
        while pos < chars.len() && !chars[pos].is_alphanumeric() && chars[pos] != '_' {
            pos += 1;
        }
        
        // 単語の終端まで移動
        while pos < chars.len() && (chars[pos].is_alphanumeric() || chars[pos] == '_') {
            pos += 1;
        }
        
        pos
    }
    
    /// 文字列の指定位置から単語の境界を検索（後方）
    pub fn find_word_boundary_backward(text: &str, position: usize) -> usize {
        let chars: Vec<char> = text.chars().collect();
        let mut pos = position;
        
        if pos > 0 {
            pos -= 1;
            
            // 現在位置が単語文字でない場合、単語文字まで移動
            while pos > 0 && !chars[pos].is_alphanumeric() && chars[pos] != '_' {
                pos -= 1;
            }
            
            // 単語の開始まで移動
            while pos > 0 && (chars[pos].is_alphanumeric() || chars[pos] == '_') {
                pos -= 1;
            }
            
            // 単語文字でない位置で止まった場合、1つ進める
            if pos < chars.len() && !chars[pos].is_alphanumeric() && chars[pos] != '_' {
                pos += 1;
            }
        }
        
        pos
    }
}

/// テキスト編集可能なトレイト
pub trait EditableText {
    fn text(&self) -> &str;
    fn cursor_position(&self) -> usize;
    fn set_text_and_cursor(&mut self, text: String, cursor: usize);
    
    /// 文字を挿入
    fn insert_char(&mut self, ch: char) {
        let (new_text, new_cursor) = TextEditor::insert_char_at(
            self.text(), 
            self.cursor_position(), 
            ch
        );
        self.set_text_and_cursor(new_text, new_cursor);
    }
    
    /// 前方削除
    fn delete_char_forward(&mut self) {
        let (new_text, new_cursor) = TextEditor::delete_char_forward_at(
            self.text(), 
            self.cursor_position()
        );
        self.set_text_and_cursor(new_text, new_cursor);
    }
    
    /// 後方削除
    fn delete_char_backward(&mut self) {
        let (new_text, new_cursor) = TextEditor::delete_char_backward_at(
            self.text(), 
            self.cursor_position()
        );
        self.set_text_and_cursor(new_text, new_cursor);
    }
    
    /// 行末まで削除
    fn kill_line(&mut self) {
        let (new_text, new_cursor) = TextEditor::kill_line_at(
            self.text(), 
            self.cursor_position()
        );
        self.set_text_and_cursor(new_text, new_cursor);
    }
    
    /// 行全体をクリア
    fn clear_line(&mut self) {
        let (new_text, new_cursor) = TextEditor::clear_line();
        self.set_text_and_cursor(new_text, new_cursor);
    }
    
    /// カーソルを行頭へ移動
    fn move_cursor_to_beginning(&mut self) {
        self.set_text_and_cursor(self.text().to_string(), 0);
    }
    
    /// カーソルを行末へ移動
    fn move_cursor_to_end(&mut self) {
        let end_pos = self.text().chars().count();
        self.set_text_and_cursor(self.text().to_string(), end_pos);
    }
    
    /// カーソルを前の単語へ移動
    fn move_cursor_word_backward(&mut self) {
        let new_cursor = TextEditor::find_word_boundary_backward(
            self.text(), 
            self.cursor_position()
        );
        self.set_text_and_cursor(self.text().to_string(), new_cursor);
    }
    
    /// カーソルを次の単語へ移動
    fn move_cursor_word_forward(&mut self) {
        let new_cursor = TextEditor::find_word_boundary_forward(
            self.text(), 
            self.cursor_position()
        );
        self.set_text_and_cursor(self.text().to_string(), new_cursor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_char() {
        let (result, cursor) = TextEditor::insert_char_at("hello", 2, 'X');
        assert_eq!(result, "heXllo");
        assert_eq!(cursor, 3);
    }

    #[test]
    fn test_delete_char_forward() {
        let (result, cursor) = TextEditor::delete_char_forward_at("hello", 1);
        assert_eq!(result, "hllo");
        assert_eq!(cursor, 1);
    }

    #[test]
    fn test_delete_char_backward() {
        let (result, cursor) = TextEditor::delete_char_backward_at("hello", 2);
        assert_eq!(result, "hllo");
        assert_eq!(cursor, 1);
    }

    #[test]
    fn test_kill_line() {
        let (result, cursor) = TextEditor::kill_line_at("hello world", 5);
        assert_eq!(result, "hello");
        assert_eq!(cursor, 5);
    }

    #[test]
    fn test_word_boundaries() {
        let text = "hello_world test";
        assert_eq!(TextEditor::find_word_boundary_forward(text, 0), 11); // "hello_world"
        assert_eq!(TextEditor::find_word_boundary_forward(text, 12), 16); // "test"
        
        assert_eq!(TextEditor::find_word_boundary_backward(text, 16), 12);
        assert_eq!(TextEditor::find_word_boundary_backward(text, 11), 0);
    }
}